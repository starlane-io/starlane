use crate::cache::CacheLayout;
use crate::remote::RemoteRepo;
use crate::repo::Repo;
use crate::zip::{unzip_from_binary_to_temp, ZipError};
use crate::{get_starlane_package_cache, PackageErr};
use starlane_space::types::specific::Slice;
use std::path::PathBuf;
use strum_macros::Display;
use tempfile::TempDir;
use thiserror::Error;
use tokio::sync::oneshot;

#[derive(Clone)]
pub struct Downloader {
    path: PathBuf,
    tx: tokio::sync::mpsc::Sender<DownloadRequest>,
}

#[derive(Debug, Error, Display)]
pub enum DownloadErr {
    Internal,
    NotFound,
    PackageErr(#[from] PackageErr),
    ZipErr(#[from] ZipError),
    IoErr(#[from] tokio::io::Error),
}

impl Default for Downloader {
    fn default() -> Self {
        let path = PathBuf::from(get_starlane_package_cache());
        let repo = RemoteRepo::default();
        Self::new(path, repo)
    }
}

impl Downloader {
    pub fn new(path: PathBuf, repo: impl Repo + 'static) -> Self {
        let (tx, rx) = tokio::sync::mpsc::channel(100);
        DownloadRunner::new(path.clone(), repo, rx, None);
        Self { path, tx }
    }

    pub fn temp(repo: RemoteRepo) -> Self {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().to_path_buf();
        let (tx, rx) = tokio::sync::mpsc::channel(100);

        DownloadRunner::new(path.clone(), repo, rx, Some(tmp));
        Self { path, tx }
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// return () meaning that the file is where it should be
    pub async fn download(&self, slice: &Slice) -> Result<(), DownloadErr> {
        let (request, rtn) = DownloadRequest::new(slice.clone());
        self.tx.send(request).await.unwrap();
        rtn.await.map(|r| ()).map_err(|_| DownloadErr::Internal)
    }
}

pub struct DownloadRequest {
    slice: Slice,
    tx: oneshot::Sender<Result<(), DownloadErr>>,
}

impl DownloadRequest {
    pub fn new(slice: Slice) -> (Self, oneshot::Receiver<Result<(), DownloadErr>>) {
        let (tx, rx) = oneshot::channel();
        (Self { slice, tx }, rx)
    }
}

struct DownloadRunner {
    layout: CacheLayout,
    repo: Box<dyn Repo>,
    tmp: Option<TempDir>,
    rx: tokio::sync::mpsc::Receiver<DownloadRequest>,
}

impl DownloadRunner {
    pub fn new(
        path: PathBuf,
        repo: impl Repo + 'static,
        rx: tokio::sync::mpsc::Receiver<DownloadRequest>,
        tmp: Option<TempDir>,
    ) {
        let repo = Box::new(repo);
        let layout = CacheLayout { path };
        let mut runner = Self {
            layout,
            repo,
            rx,
            tmp,
        };
        runner.start();
        println!("returning from DownloadRunner::new");
    }

    pub fn start(mut self) {
        println!("Download Runner STARTED !");
        tokio::spawn(async move {
            println!("Download runner running!");
            while let Some(request) = self.rx.recv().await {
                let result = self.download_slice(&request.slice).await;
                request.tx.send(result).unwrap();
            }
        });
    }

    async fn download_slice(&self, slice: &Slice) -> Result<(), DownloadErr> {
        println!("START DOWNLOAD SLICE: {}", slice);
        let path = self.layout.slice_path(slice);
        println!("slice path: '{}'", path.to_str().unwrap());
        if path.exists() {
            println!("slice exists!");
            return Ok(());
        }

        let data = self.repo.get_slice(slice).await?;
        println!("got slice data.  {} ", data.len());
        let dir = unzip_from_binary_to_temp(data.as_slice()).unwrap();
        println!("slice data saved to: '{}'", dir.path().to_str().unwrap());

        println!("unzipped slice....");
        tokio::fs::create_dir_all(path.clone()).await?;
        println!(
            "created SLICE cache directory: '{}'",
            path.to_str().unwrap()
        );
        tokio::fs::rename(dir.path(), path.clone()).await?;
        println!("slice renamed.... to {}", path.display());
        Ok(())
    }
}
