use std::fmt::Debug;
use std::hash::Hash;
use thiserror::Error;
use starlane_space::types::scope::Segment;
use starlane_space::types::specific::{Release, Specific};

#[cfg(feature="create")]
pub mod create;

/// a convenience struct for understanding and
/// managing the anatomy of a package structure.
pub struct Package {
    /// exact release of this package
    release: Release,
    /// this is the default/main slice if no slice is specified
    main: Slice,
    /// [Package] may have any number of child slices other than [Package::main]
    slices: Vec<Slice>
}

#[derive(Error, Debug)]
pub enum PackageErr {
    #[error("Main package slice must be named 'main'")]
    IllegalMain,
    #[error("subslices may not be named 'main'")]
    MainSubSlice
}

impl Package {
    pub fn verify(&self) -> Result<(),PackageErr> {
        if !self.main.is_main() {
            return Err(PackageErr::IllegalMain);
        }

        for slice in self.slices {
            slice.verify()?;
        }

        Ok(())
    }
}


/// a [Slice] is NOT a [Directory] but an independent part of a [Package] that
/// can be downloaded separately and independently. For example imagine a package with slices:
/// ```md
/// * package `uberscott.com:website:1.2.3` with slices:
///   - frontend
///   - backend
///   - common
///   - cdn
/// ```
/// It behooves the developer to wrap all aspects of a release into one [Package] for consistency,
/// however, the `frontend` for example may not need to download the entire `backend` slice
/// to operate and likewise the `cdn` (Content Delivery Network) slice may be conveniently packaged
/// with the versions its meant to work with...
///
pub struct Slice {
    /// the identity of this slice
    segment: Segment,
    ///
    children: Vec<Entity>
}

impl Slice {
    pub fn is_main(&self) -> bool {
        self.segment.is_main()
    }
    pub fn verify(&self) -> Result<(),PackageErr> {
        for child in &self.children {
            match child {
                Entity::File(file) => { }
                Entity::Slice(slice) => {
                    if slice.is_main() {
                        return Err(PackageErr::MainSubSlice);
                    }
                    slice.verify()?;
                }
            }
        }

        Ok(())
    }
}

pub struct Directory{
    name: String,
    children: Vec<FileEntity>
}

pub enum Entity {
    File(FileEntity),
    Slice(Slice)
}
pub enum FileEntity {
    File(String),
    Directory(Directory)
}


#[cfg(test)]
mod tests {
    use super::Package;
}

