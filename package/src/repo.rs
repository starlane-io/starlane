use std::fmt::Display;
use crate::create::{PackErr, PackageLayout};
use crate::zip::zip_slice_dir_to;
use crate::{PackageErr, PublishObserver};
use async_trait::async_trait;
use starlane_base::env::get_starlane_package_source;
use starlane_space::types::scope::SlicePath;
use starlane_space::types::specific::Slice;
use std::{fs, io};
use std::path::PathBuf;
use std::sync::Arc;
use reqwest::{Error, Response, StatusCode};
use tempfile::TempDir;
use thiserror::Error;
use tokio::io::AsyncReadExt;
use crate::test::MockPublishObserver;

#[async_trait]
pub trait Repo : Send+Sync{
    async fn get_slice(&self, slice: &Slice) -> Result<Vec<u8>, PackageErr>;
    async fn publish(
        &self,
        layout: &PackageLayout
    ) -> Result<(), PackageErr>;

    async fn status(&self) -> RepoStatus;
}

#[derive(Error,Debug)]
pub enum RepoStatus {
    #[error("Unknown")]
    Unknown,
    #[error("Ready")]
    Ready,
    #[error("Panic({0})")]
    Panic(#[from] RepoPanic)
}

#[derive(Error,Debug)]
pub enum RepoPanic {
    #[error("Unreachable ({0})")]
    Unreachable(String),
    #[error("Unauthorized")]
    Unauthorized,
    #[error("Timeout")]
    Timeout,
    #[error("StatusCode({0})")]
    StatusCode(reqwest::StatusCode),
    #[error("Panic({0})")]
    Error(String)
}

impl From<Result<reqwest::Response,reqwest::Error>> for RepoStatus{
    fn from(result: Result<Response, Error>) -> Self {
        match result {
            Ok(response) => {
                response.into()
            }
            Err(err) =>  {
                err.into()
            }
        }
    }
}

impl From<reqwest::Error> for RepoStatus {
    fn from(err: Error) -> Self {
        Self::Panic(err.into())
    }
}

impl From<reqwest::Error> for RepoPanic {
    fn from(err: Error) -> Self {
        RepoPanic::Unreachable(err.to_string())
    }
}

impl From<reqwest::Response> for RepoStatus {
    fn from(response: Response) -> Self {
        /// right now we just go by StatusCode
        response.status().into()
    }
}

impl From<reqwest::StatusCode> for RepoStatus {
    fn from(code: StatusCode) -> Self {
        match code.is_success() {
            true => RepoStatus::Ready,
            false => RepoStatus::Panic(code.into())
        }
    }
}

impl From<reqwest::StatusCode> for RepoPanic {
    fn from(code: reqwest::StatusCode) -> Self {
        match code {
            StatusCode::UNAUTHORIZED => Self::Unauthorized,
            StatusCode::REQUEST_TIMEOUT=> Self::Timeout,
            code => code.into()
        }
    }
}

#[derive(Clone)]
pub enum RepoDir {
    Path(PathBuf),
    Temp(Arc<TempDir>)
}

impl From<TempDir> for RepoDir {
    fn from(dir: TempDir) -> Self {
        Self::Temp(Arc::new(dir))
    }
}

impl From<PathBuf> for RepoDir {
    fn from(path: PathBuf) -> Self {
        Self::Path(path)
    }
}

impl RepoDir {
    pub fn temp() -> Result<Self,io::Error> {
        TempDir::new().map(Into::into)
    }

    pub fn path( path: PathBuf ) -> Self {
        Self::Path(path)
    }

    pub fn as_path_buf(&self) -> PathBuf {
        match self {
            RepoDir::Path(path) =>path.clone(),
            RepoDir::Temp(temp) => temp.path().to_path_buf()
        }
    }
}

#[derive(Clone)]
pub struct SourceRepo {
    root: RepoDir,
}

impl Default for SourceRepo {
    fn default() -> Self {
        Self {
            root: get_starlane_package_source().into()
        }
    }
}

impl SourceRepo {
    pub fn new(path: PathBuf) -> Self {
        if !path.exists() {
            fs::create_dir_all(&path).unwrap();
        }
        Self { root: path.into() }
    }

    pub fn nuke(&self) -> Result<(), PackageErr> {
        fs::remove_dir_all(&self.root.as_path_buf())?;
        fs::create_dir_all(&self.root.as_path_buf())?;
        Ok(())
    }

    pub fn temp() -> Self {
            Self {
                root: RepoDir::temp().unwrap()
            }
    }

    #[cfg(test)]
    pub async fn mock() -> Self {
        let layout = crate::test::package_layout();
        let source = Self {
            root: RepoDir::temp().unwrap()
        };
        source.publish(&layout).await.unwrap();
        source
    }



}

#[async_trait]
impl Repo for SourceRepo {
    async fn get_slice(&self, slice: &Slice) -> Result<Vec<u8>, PackageErr> {
        let path = self.root.as_path_buf().join(slice.to_path().to_str().unwrap());

        if !path.exists() {
            return Err(PackErr::SliceNotFound(slice.to_string()).into());
        }

        let mut file = tokio::fs::File::open(&path).await?;
        let mut contents = Vec::new();
        file.read_to_end(&mut contents).await?;

        Ok(contents)
    }

    async fn publish(
        &self,
        layout: &PackageLayout,
    ) -> Result<(), PackageErr>
    {
        let release_dir = self.root.as_path_buf().join(layout.release().to_path());
        fs::create_dir_all(release_dir.clone())?;
        /// first zip main/root which is a special case
        let main_target = release_dir.join(SlicePath::root().filename());
        zip_slice_dir_to(&layout.path, main_target).map_err(Into::<PackErr>::into)?;

        let paths = layout.gather_slice_paths();
        for p in paths {
            let source = layout.path.join(p.as_path());
            let target = release_dir.join(p.filename());
            zip_slice_dir_to(source, target).map_err(Into::<PackErr>::into)?;
        }

        Ok(())
    }

    async fn status(&self) -> RepoStatus {
       RepoStatus::Ready
    }
}
