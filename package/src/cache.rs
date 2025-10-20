use std::path::PathBuf;
use std::sync::Arc;
use strum_macros::Display;
use tempfile::TempDir;
use thiserror::Error;
use starlane_base::env;
use starlane_space::types::specific::{PackFile, Slice};
use crate::download::{DownloadErr, Downloader};
use crate::PackageErr;
use crate::remote::RemoteRepo;
use crate::repo::Repo;

#[derive(Clone)]
pub struct PackageCache {
    tmp: Option<Arc<TempDir>>,
    pub layout: CacheLayout,
    pub downloader: Downloader
}

impl PackageCache {

    pub fn temp(  ) -> Self {
        let repo = RemoteRepo::default();
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().to_path_buf();
        let layout = CacheLayout::new(path.clone());
        let tmp = Some(Arc::new(tmp));
        let downloader = Downloader::new(path.clone(), repo);
        Self {
            tmp,
            layout,
            downloader
        }
    }

    pub fn is_file_cached(&self, file: &PackFile) -> bool {
        self.layout.file_path(file).exists()
    }
    pub async fn get_file(&self, file: &PackFile) -> Result<Vec<u8>,CacheErr> {
        let path = self.layout.file_path(file);
        if !path.exists() {
println!("downloading slice: {}", file.slice() );
            self.downloader.download(file.slice()).await?;
        }
println!("FILE EXISTS? {} -> {}", path.exists(), path.to_str().unwrap() );
        let data = tokio::fs::read(path).await?;
        Ok(data)
    }
}


/// a utility struct for finding files in the cache using
/// convension.  Used by DownloadRunner and PackageCache
#[derive(Clone)]
pub(crate) struct CacheLayout {
    pub path: PathBuf
}

impl CacheLayout {

    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
    pub fn slice_path(&self, slice: &Slice ) -> PathBuf {
        self.path.join(slice.to_path())
    }
    pub fn file_path(&self, file: &PackFile) -> PathBuf {
        self.path.join(file.to_path())
    }
}


#[derive(Debug,Error)]
pub enum CacheErr {
    #[error("Not Found")]
    NotFound,
    #[error("Error downloading slice: {0}")]
    DownloadErr(#[from] DownloadErr),
    #[error("{0}")]
    PackageErr(#[from] PackageErr),
    #[error("{0}")]
    IoErr(#[from] tokio::io::Error)
}