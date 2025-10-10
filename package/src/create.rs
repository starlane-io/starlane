use crate::{Directory, Entity, FileEntity, PackageStructure, Slice};
use starlane_space::err::ParseErrs0;
use starlane_space::types::scope::Segment;
use std::path::{PathBuf, StripPrefixError};
use std::str::FromStr;
use std::{fs, io};
use std::collections::HashMap;
use tempfile::NamedTempFile;
use thiserror::Error;
use crate::server::PackObserver;
use crate::zip::{zip_directory_to_temp, ZipError};

pub struct PackageDirectoryStructure {
    pub root: PathBuf,
    pub structure: PackageStructure,
}

impl PackageDirectoryStructure {
    pub fn create(root: &PathBuf, observer: &mut dyn PackObserver) -> Result<Self, PackErr> {

        observer.start_pack( &root);

        observer.start_verify_layout();
        
        /// should only be called on a directory that is directly
        /// under a Slice (because it could be a sub-slice)
        fn walk_entity(dir: &PathBuf) -> Result<Entity, PackErr> {
            if has_slice_file(dir) {
                slice_name(dir)?;
                Ok(Entity::Slice(walk_slice(dir)?))
            } else {
                println!("DIR  : {}", dir.display());
                Ok(Entity::Directory(walk_dir(dir)?))
            }
        }

        fn walk_slice(dir: &PathBuf) -> Result<Slice, PackErr> {
            let name = slice_name(&dir)?;
            let mut slice = Slice::new(name);
            for entry in fs::read_dir(dir)? {
                let path = entry?.path();
                if path.is_file() {
                    if !ignore(&path) {
                        let filename = file_name(&path)?;
                        slice
                            .directory
                            .children
                            .insert(filename.clone(), FileEntity::File(filename));
                    }
                } else {
                    match walk_entity(&path)? {
                        Entity::Directory(directory) => {
                            slice.directory.children.insert(directory.name.clone(),FileEntity::Directory(directory));
                        }
                        Entity::Slice(s) => {
                            slice.slices.insert(s.segment.clone(), s);
                        }
                    }
                }
            }
            Ok(slice)
        }

        /// walkdir
        fn walk_dir(dir: &PathBuf) -> Result<Directory, PackErr> {
            let name = file_name(dir)?;
            let mut directory = Directory::new(name);
            for entry in fs::read_dir(dir)? {
                let path = entry?.path();
                if !ignore(&path) {
                    if path.is_file() {
                        let filename = file_name(&path)?;
                        directory.children.insert(filename.clone(),FileEntity::File(filename));
                    } else {
                        let subdir = walk_dir(&path)?;
                        directory.children.insert(subdir.name.clone(),FileEntity::Directory(subdir));
                    }
                }
            }
            Ok(directory)
        }

        let mut slices =HashMap::new();

        for entry in fs::read_dir(root)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let slice = walk_slice(&path)?;
                observer.found_slice(slice.segment.to_string().as_str());
                slices.insert(slice.segment.clone(), slice);
            }
        }

        let mut structure = PackageStructure::new(slices);
        structure.finalize();
        
        
        let pds = Self {
            root: root.clone(),
            structure,
        };

        observer.end_pack();
        Ok(pds)
    }

    pub fn diagnose(&self) {
        self.diagnose_indent(0);
    }

    pub fn diagnose_indent(&self,mut spaces:usize ) {
        let indent = " ".repeat(spaces);
        println!("{indent}{}[PackageDirectoryStructure]",self.root.display());
        self.structure.diagnose_indent(spaces+2);
    }


    pub fn zip(&self) -> Result<NamedTempFile, PackErr> {
        use crate::zip::zip_directory_to_temp;
        let path = zip_directory_to_temp(self.root.clone())?;
        Ok(path)
    }
}

#[derive(Debug, Error)]
pub enum PackErr {
    #[error("{0}")]
    SliceNameErr(ParseErrs0),
    #[error("{0}")]
    IoErr(std::io::Error),
    #[error("{0}")]
    StripPrefixErr(StripPrefixError),
    #[error("Invalid slice name: '{0}'")]
    InvalidSliceName(String),
    #[error("ZipErr: {0}")]
    ZipError(ZipError),

}

impl From<ZipError> for PackErr {
    fn from(err: ZipError) -> Self {
        PackErr::ZipError(err)
    }
}

impl From<io::Error> for PackErr {
    fn from(err: io::Error) -> Self {
        PackErr::IoErr(err)
    }
}

impl From<ParseErrs0> for PackErr {
    fn from(errs: ParseErrs0) -> Self {
        PackErr::SliceNameErr(errs)
    }
}

impl From<StripPrefixError> for PackErr {
    fn from(errs: StripPrefixError) -> Self {
        PackErr::StripPrefixErr(errs)
    }
}

fn has_slice_file(path: &PathBuf) -> bool {
    if path.is_dir() {
        let slice_path = path.join(".slice");
        slice_path.is_file()
    } else {
        false
    }
}

fn slice_name(path: &PathBuf) -> Result<Segment, PackErr> {
    Ok(Segment::from_str(file_name(path)?.as_str())?)
}

fn file_name(path: &PathBuf) -> Result<String, PackErr> {
    Ok(path
        .file_name()
        .ok_or(PackErr::InvalidSliceName(format!("{}", path.display())))?
        .to_str()
        .ok_or(PackErr::InvalidSliceName(format!("{}", path.display())))?
        .to_string())
}

fn stringify(path: &PathBuf) -> Result<String, PackErr> {
    Ok(format!("{}", path.display()).to_string())
}

fn ignore(path: &PathBuf) -> bool {
    match file_name(path).unwrap().as_str() {
        ".slice" => true,
        _ => false,
    }
}



