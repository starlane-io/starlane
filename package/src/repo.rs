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
    async fn get_slice(&self, slice: &Slice) -> Result<Vec<u8>, PackageErr>;
    async fn submit<P>(
        &self,
        layout: &PackageLayout,
        observer: P,
    ) -> anyhow::Result<(), PackageErr>
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
        if !path.exists() {
            fs::create_dir_all(&path).unwrap();
        }
        Self { root: path }
    }

    pub fn nuke(&self) -> Result<(), PackageErr> {
        fs::remove_dir_all(&self.root)?;
        fs::create_dir_all(&self.root)?;
        Ok(())
    }

    pub fn temp() -> (Self, TempDir) {
        let dir = TempDir::new().unwrap();
        (
            Self {
                root: dir.path().to_path_buf(),
            },
            dir,
        )
    }

    #[cfg(test)]
    pub fn mock() -> (Self, TempDir) {
        let dir = TempDir::new().unwrap();
        
        let layout = crate::test::package_layout();
        let source = Self {
            root: dir.path().to_path_buf(),
        };
        source.submit(layout).unwrap();
        (
            source,
            dir,
        )
    }
    
    

    pub fn submit(&self, package: PackageLayout) -> Result<(), PackErr> {
        let release_dir = self.root.join(package.release().to_path());
        fs::create_dir_all(release_dir.clone())?;
        /// first zip main/root which is a special case
        let main_target = release_dir.join(SlicePath::root().filename());
        zip_slice_dir_to(&package.path, main_target)?;

        let paths = package.gather_slice_paths();
        for p in paths {
            let source = package.path.join(p.as_path());
            let target = release_dir.join(p.filename());
            zip_slice_dir_to(source, target)?;
        }

        Ok(())
    }

    pub async fn get_slice(&self, slice: &Slice) -> Result<Vec<u8>, PackErr> {
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
