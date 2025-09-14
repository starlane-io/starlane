use crate::{Directory, Entity, FileEntity, Package, Slice};
use starlane_space::err::ParseErrs0;
use starlane_space::types::scope::Segment;
use std::path::{PathBuf, StripPrefixError};
use std::str::FromStr;
use std::{fs, io};
use thiserror::Error;
use walkdir::Error;
use starlane_space::types::specific::Release;

#[derive(Debug, Error)]
pub enum PackErr {
    #[error("{0}")]
    SliceNameErr(ParseErrs0),
    #[error("{0}")]
    WalkDirErr(Error),
    #[error("{0}")]
    IoErr(std::io::Error),
    #[error("{0}")]
    StripPrefixErr(StripPrefixError),
    #[error("Invalid slice name: '{0}'")]
    InvalidSliceName(String),
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
impl From<walkdir::Error> for PackErr {
    fn from(errs: walkdir::Error) -> Self {
        PackErr::WalkDirErr(errs)
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

pub fn create(release: String, dir: &PathBuf) -> Result<Package, PackErr> {
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
                slice
                    .directory
                    .files
                    .push(FileEntity::File(file_name(&path)?));
            } else {
                match walk_entity(&path)? {
                    Entity::Directory(directory) => {
                        slice.directory.files.push(FileEntity::Directory(directory));
                    }
                    Entity::Slice(s) => {
                        slice.slices.push(s);
                    }
                }
            }
        }
        Ok(slice)
    }

    /// walkdir
    fn walk_dir(dir: &PathBuf) -> Result<Directory, PackErr> {
        let name = dir.display().to_string();
        let mut directory = Directory::new(name);
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
            if path.is_file() {
                directory.files.push(FileEntity::File(file_name(&path)?));
            } else {
                let subdir = walk_dir(&path)?;
                directory.files.push(FileEntity::Directory(subdir));
            }
        }
        Ok(directory)
    }

    let mut slices = Vec::<Slice>::new();

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let slice = walk_slice(&path)?;
            slices.push(slice);
        }
    }

    let package = Package::new(release, slices);

    package.diagnose();
    Ok(package)
}

#[cfg(test)]
mod test {
    use crate::create::create;
    use std::path::PathBuf;
    use std::str::FromStr;
    use starlane_space::types::specific::Release;

    #[test]
    pub fn test_create() {
        let path = PathBuf::from_str("test/package-layout-example").unwrap();
        create("uberscott.io:mystuff:1.3.5".to_string(),&path);
    }
}
