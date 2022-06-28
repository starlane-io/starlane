use alloc::sync::Arc;
use alloc::vec::Vec;
use std::io;
use std::prelude::rust_2021::String;
use std::io::Error;
use anyhow::anyhow;
use mesh_portal::error::MsgErr;

pub type Res<R>=Result<R,MsgErr>;

pub trait FileAccess {
    fn rmdir(&self, path: &str ) -> Res<()>;
    fn list(&self, path: &str ) -> Res<Vec<String>>;
    fn read(&self, path: &str ) -> Res<Vec<u8>>;
    fn write(&self, path: &str, data: Vec<u8>) -> Res<()>;
    fn unzip(&self, source: &str, targer: &str) -> Res<()>;
    fn mkdir(&self, path: &str) -> Res<()>;
    fn exists(&self, path: &str) -> Res<()>;
    fn remove(&self, path: &str) -> Res<()>;
}

#[cfg(feature = "local-filesystem")]
pub mod local{
    use alloc::vec::Vec;
    use core::fmt::Error;
    use std::{format, fs, io, vec};
    use crate::file::{FileAccess, Res};
    use std::path::Path;
    use std::path::PathBuf;
    use std::prelude::rust_2021::{String, ToString};
    use mesh_portal::error::MsgErr;

    pub struct LocalFileAccess {
        root: PathBuf
    }

    impl LocalFileAccess {
        fn path( &self, with: &str ) -> Res<PathBuf> {
            let mut path = self.root.clone();
            path.push( with );
            let path = path.as_path();
            Ok(path.to_path_buf())
        }
    }

    impl FileAccess for LocalFileAccess {
        fn rmdir(&self, path: &str) -> Res<()> {
            Ok(fs::remove_dir_all(path)?)
        }

        fn list(&self, path: &str) -> Res<Vec<String>> {
            let path = self.path(path)?;
            let mut rtn = vec![];
            for entry in fs::read_dir(path)? {
                rtn.push(entry?.path().to_str().ok_or("expected to be able to convert path to str")?.to_string());
            }
            Ok(rtn)
        }

        fn read(&self, path: &str) -> Res<Vec<u8>> {
            let path = self.path(path)?;
            Ok(fs::read(path)?)
        }

        fn write(&self, path: &str, data: Vec<u8>) -> Res<()> {
            let path = self.path(path)?;
            Ok(fs::write(path,data)?)
        }

        fn unzip(&self, source: &str, target: &str) -> Res<()> {
            let source = self.path(source)?;
            let target = self.path(target)?;
            let source = fs::File::open(source)?;
            let mut archive = zip::ZipArchive::new(source).map_err( |r| MsgErr::new(500,r.to_string().as_str()))?;

            for i in 0..archive.len() {
                let mut zip_file = archive.by_index(i).map_err( |r| MsgErr::new(500,r.to_string().as_str()))?;
                let mut target = target.to_path_buf();
                target.push(zip_file.name() );
                if zip_file.is_dir() {
                    fs::create_dir_all(target)?;
                } else {
                    target.push(zip_file.name() );
                    let parent = target.parent().ok_or("expected path").map_err( |r| MsgErr::new(500,r.to_string().as_str()))?;
                    fs::create_dir_all(parent)?;
                    let mut file = fs::File::create(target)?;
                    std::io::copy(&mut zip_file, &mut file)?;
                }
            }
            Ok(())
        }

        fn mkdir(&self, path: &str) -> Res<()> {
            Ok(fs::create_dir_all(path)?)
        }

        fn exists(&self, path: &str) -> Res<()> {
            let path = self.path(path)?;
            if path.exists() {
                Ok(())
            } else {
                Err("path does not exist".into())
            }
        }

        fn remove(&self, path: &str) -> Res<()> {
            let path = self.path(path)?;
            Ok(fs::remove_file(path)?)
        }
    }
}


