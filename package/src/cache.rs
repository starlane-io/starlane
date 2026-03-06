use crate::download::{DownloadErr, Downloader};
use crate::repo::Repo;
use crate::PackageErr;
use starlane_space::types::specific::{PackFile, Slice};
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;
use thiserror::Error;

#[derive(Clone)]
pub struct PackageCache {
    tmp: Option<Arc<TempDir>>,
    pub layout: CacheLayout,
    pub downloader: Downloader
}

impl PackageCache {

    /// create a temporary unique cache directory that will be deleted on process termination
    pub fn temporary(repo: impl Repo+'static) -> Self {
        Self::unique_with_keep(repo, false)
    }

    /// create a unique cache directory that will not be deleted when the process terminates.
    /// Developers should use this constructor if they are running unit tests and wish to examine
    /// the contents of the cache after the test.
    pub fn unique(repo: impl Repo+'static) -> Self {
        Self::unique_with_keep(repo,true)
    }

    /// creates a unique cache directory with a `keep` flag to preserve cache directory after the process terminates.
    pub fn unique_with_keep( repo: impl Repo+'static, keep: bool ) -> Self {
        let mut tmp = TempDir::new().unwrap();

        if keep {
            tmp.disable_cleanup(true);
            println!("Temp Cache Location: '{}'", tmp.path().to_str().unwrap() )
        }

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
println!("EXPECTED PATH FILE: '{}'", path.to_str().unwrap());
        if !path.exists() {
println!("downloading slice: {} for cache file: '{}'", file.slice(), path.to_str().unwrap());
            self.downloader.download(file.slice()).await?;
        }

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
    #[error("Download operation cache directory miss for file: {0} with expected cache directory of: {1}")]
    DownloadCacheMiss(PackFile,PathBuf),
    #[error("Error downloading slice: {0}")]
    DownloadErr(#[from] DownloadErr),
    #[error("{0}")]
    PackageErr(#[from] PackageErr),
    #[error("{0}")]
    IoErr(#[from] tokio::io::Error)
}