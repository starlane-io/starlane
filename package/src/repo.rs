use crate::create::{PackErr, PackageLayout};
use crate::zip::zip_slice_dir_to;
use crate::{PackageErr, PublishObserver};
use async_trait::async_trait;
use starlane_base::env::get_starlane_package_source;
use starlane_space::types::scope::SlicePath;
use starlane_space::types::specific::Slice;
use std::fs;
use std::path::PathBuf;

#[async_trait]
pub trait Repo {
    async fn get_slice(&self, specific: &Slice) -> Result<Vec<u8>, PackageErr>;
    async fn submit<P>(&self, pds: &PackageLayout, observer: P) -> anyhow::Result<(), PackageErr> where P: PublishObserver+Send+Sync;
}

pub struct SourceRepo {
   root: PathBuf
}

impl Default for SourceRepo {
    fn default() -> Self {
        Self {
            root: PathBuf::from(get_starlane_package_source())
        }
    }
}

impl SourceRepo {
    pub fn new(path: PathBuf) -> Self {
        Self {
            root: path
        }
    }

    pub fn save_package( &self, package: PackageLayout ) -> Result<(),PackErr>{
        let release_dir = self.root.join(package.release_directory());
        fs::create_dir(release_dir.clone())?;
        /// first zip main/root which is a special case
        let main_target= release_dir.join(SlicePath::main().filename());
        zip_slice_dir_to(&package.root, main_target)?;

        let paths = package.gather_slice_paths();
        for p in paths {
            let source = package.root.join(p.as_path());
            let target = release_dir.join(p.filename());
            zip_slice_dir_to(source, target)?;
        }

        Ok(())
    }

    pub fn get_slice_path(&self, specific: Slice) -> Result<PathBuf, String> {
        let release = specific.release().to_string().replace(":","_");
        let release_path = self.root.join(release);

        let slice_path = release_path.join(specific.slices().filename()).join(".zip");

        Ok(slice_path)
    }

}