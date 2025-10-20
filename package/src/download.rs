use crate::remote::RemoteRepo;
use crate::repo::Repo;
use crate::zip::{unzip_from_binary_to_temp, ZipError};
use crate::PackageErr;
use starlane_space::types::specific::Slice;
use std::path::PathBuf;
use strum_macros::Display;
use tempfile::TempDir;
use thiserror::Error;
use tokio::sync::oneshot;

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

impl Downloader {
    pub fn new(path: PathBuf, repo: RemoteRepo) -> Self {
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

    /// return () meaning
    pub async fn download(&self, slice: &Slice) -> Result<(), DownloadErr> {
        let (request, rtn) = DownloadRequest::new(slice.clone());
println!("sending download request");
        self.tx.send(request).await.unwrap();
        rtn.await.map_err(|_| DownloadErr::Internal)?;
        Ok(())
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
    path: PathBuf,
    repo: RemoteRepo,
    tmp: Option<TempDir>,
    rx: tokio::sync::mpsc::Receiver<DownloadRequest>,
}

impl DownloadRunner {
    pub fn new(path: PathBuf, repo: RemoteRepo, rx: tokio::sync::mpsc::Receiver<DownloadRequest>, tmp: Option<TempDir>) {
        let mut runner = Self { path, repo, rx, tmp };
        runner.start();
        println!("returning from DownloadRunner::new");
    }

    pub fn start(mut self) {
        println!("Download Runner STARTED !");
        tokio::spawn(async move {
            println!("Download runner running!");
            while let Some(request) = self.rx.recv().await {
                println!("received download request!");
                // first test if the file is already downloaded
                request
                    .tx
                    .send(self.download_slice(&request.slice).await)
                    .unwrap();
            }
        });
    }

    async fn download_slice(&self, slice: &Slice) -> Result<(), DownloadErr> {
        let path = self.path.join(slice.to_path());
        println!("slice path: '{}'", path.to_str().unwrap());
        if path.exists() {
            println!("slice exists!");
            return Ok(());
        }

        let data = self.repo.get_slice(slice).await?;
        println!("got data....");
        let dir = unzip_from_binary_to_temp(data.as_slice())?;
        println!("unzipped slice....");
        tokio::fs::rename(dir.path(), path).await?;
        println!("slice renamed....");
        Ok(())
    }
}
