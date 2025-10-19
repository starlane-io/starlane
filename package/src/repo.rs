use crate::create::{PackErr, PackageLayout};
use crate::zip::zip_slice_dir_to;
use crate::{PackageErr, PublishObserver};
use async_trait::async_trait;
use starlane_base::env::get_starlane_package_source;
use starlane_space::types::scope::SlicePath;
use starlane_space::types::specific::Slice;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;
use tokio::io::AsyncReadExt;

#[async_trait]
pub trait Repo {
    async fn get_slice(&self, specific: &Slice) -> Result<Vec<u8>, PackageErr>;
    async fn submit<P>(&self, pds: &PackageLayout, observer: P) -> anyhow::Result<(), PackageErr>
    where
        P: PublishObserver + Send + Sync;
}

#[derive(Clone)]
pub struct SourceRepo {
    root: PathBuf,
}

impl Default for SourceRepo {
    fn default() -> Self {
        Self {
            root: PathBuf::from(get_starlane_package_source()),
        }
    }
}

impl SourceRepo {
    pub fn new(path: PathBuf) -> Self {
        Self { root: path }
    }
    
    pub fn temp() -> (Self,TempDir) {
        let dir = TempDir::new().unwrap();
        (Self {
            root: dir.path().clone().to_path_buf()
        },dir)
    }

    pub fn save_package(&self, package: PackageLayout) -> Result<(), PackErr> {
        let release_dir = self.root.join(package.release_directory());
        fs::create_dir(release_dir.clone())?;
        /// first zip main/root which is a special case
        let main_target = release_dir.join(SlicePath::main().filename());
        zip_slice_dir_to(&package.root, main_target)?;

        let paths = package.gather_slice_paths();
        for p in paths {
            let source = package.root.join(p.as_path());
            let target = release_dir.join(p.filename());
            zip_slice_dir_to(source, target)?;
        }

        Ok(())
    }

    pub async fn get_slice(&self, slice: &Slice) -> Result<Vec<u8>,PackErr> {
        let path = self.root.join(slice.to_path());
        if !path.exists() {
            return Err(PackErr::SliceNotFound(slice.to_string()));
        }

        let mut file = tokio::fs::File::open(&path).await?;
        let mut contents = Vec::new();
        file.read_to_end(&mut contents).await?;
        
        Ok(contents)
    }
}
