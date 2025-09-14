use std::ffi::OsStr;
use std::{fs, io};
use std::fs::File;
use std::path::{PathBuf, StripPrefixError};
use std::str::FromStr;
use thiserror::Error;
use walkdir::{Error, WalkDir};
use zip::ZipArchive;
use starlane_space::err::ParseErrs0;
use starlane_space::parse::{format, SkewerCase};
use starlane_space::types::scope::Segment;
use crate::{Directory, Entity, FileEntity, Package, Slice};

#[derive(Debug,Error)]
pub enum PackErr{
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

impl From<io::Error> for PackErr{
    fn from(err: io::Error) -> Self {
        PackErr::IoErr(err)
    }
}

impl From<ParseErrs0> for PackErr{
    fn from(errs: ParseErrs0) -> Self {
        PackErr::SliceNameErr(errs)
    }
}
impl From<walkdir::Error> for PackErr{
    fn from(errs: walkdir::Error) -> Self {
        PackErr::WalkDirErr(errs)
    }
}
impl From<StripPrefixError> for PackErr{
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
    let filename = path.file_name().ok_or(PackErr::InvalidSliceName(format!("{}", path.display())))?.to_str().ok_or(PackErr::InvalidSliceName(format!("{}", path.display())))?.to_string();
    Ok(Segment::from_str(filename.as_str())?)
}

fn stringify(path: &PathBuf) -> Result<String, PackErr> {
    Ok(format!("{}",path.display()).to_string())
}
pub fn create(dir: &PathBuf) -> Result<ZipArchive<File>, PackErr> {
    /// should only be called on a directory that is directly
    /// under a Slice (because it could be a sub-slice)
    fn walk_entity(dir: &PathBuf) -> Result<Entity, PackErr> {
        if has_slice_file(dir){
            slice_name(dir)?;
            Ok(Entity::Slice(walk_slice(dir)?))
        } else {
            println!("DIR  : {}", dir.display());
            Ok(Entity::File(FileEntity::Directory(walk_dir(dir)?)))
        }
    }

    fn walk_slice(dir: &PathBuf) -> Result<Slice, PackErr> {
        let name = slice_name(&dir)?;
        let mut slice = Slice::new(name);
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
            if path.is_file() {
                slice.files.push(FileEntity::File(stringify(&path)?));
            } else {
                match walk_entity(&path)? {
                    Entity::File(file) => {
                        match file {
                            FileEntity::File(file) => {
                                slice.files.push(FileEntity::File(file));
                            }
                            FileEntity::Directory(directory) => {
                                slice.files.push(FileEntity::Directory(directory));
                            }
                        }
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
        let mut directory= Directory::new(name);
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
            if path.is_file() {
                directory.files.push(FileEntity::File(stringify(&path)?));
            } else {
                let subdir = walk_dir(&path)?;
                directory.files.push(FileEntity::Directory(subdir));
            }
        }
        Ok(directory)
    }

        let mut package = Package::new();

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
println!("WALK ....{}",path.display());
                walk_entity(&path)?;
                /* let path = path.strip_prefix(dir)?;

                 if path.file_name().is_some() && path.file_name().unwrap().to_str().is_some() {
                     println!("{} -> {}", path.display(), path.file_name().unwrap().to_str().unwrap());
                 } else {
                     println!("ROOT!");
                 }

                 */
            }
        }

        /*
        for file in walkdir::WalkDir::new(dir).into_iter() {

            if path.file_name().is_some() && path.file_name().unwrap().to_str().is_some() {
                println!("{} -> {}", path.display(), path.file_name().unwrap().to_str().unwrap());
            } else {
                println!("ROOT!");
            }
        }

         */
    todo!();
}





#[cfg(test)]
mod test {
    use std::path::PathBuf;
    use std::str::FromStr;
    use crate::create::create;

    #[test]
    pub fn test_create() {
        let path = PathBuf::from_str("test/package-layout-example").unwrap();
        create(&path);
    }
}
