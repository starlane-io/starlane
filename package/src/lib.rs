use crate::create::PackErr;
use starlane_space::parse::SkewerCase;
use starlane_space::types::scope::Segment;
use starlane_space::types::specific::{Release, Specific};
use std::fmt::Debug;
use std::hash::Hash;
use std::str::FromStr;
use thiserror::Error;

#[cfg(feature = "create")]
pub mod create;

/// a convenience struct for understanding and
/// managing the anatomy of a package structure.
pub struct Package {
    /// this is the default/main slice if no slice is specified
    main: Slice,
}

#[derive(Error, Debug)]
pub enum PackageErr {
    #[error("Main package slice must be named 'main'")]
    IllegalMain,
    #[error("subslices may not be named 'main'")]
    MainSubSlice,
    #[cfg(feature = "create")]
    #[error("{0}")]
    PackErr(PackErr),
}

impl From<PackErr> for PackageErr {
    fn from(err: PackErr) -> Self {
        PackageErr::PackErr(err)
    }
}

impl Package {
    pub fn new() -> Self {
        let main = Slice::new_main();
        Self { main }
    }

    pub fn verify(&self) -> Result<(), PackageErr> {
        if !self.main.is_main() {
            return Err(PackageErr::IllegalMain);
        }

        self.main.verify_children()?;

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
    slices: Vec<Slice>,
    files: Vec<FileEntity>,
}

impl Slice {
    pub fn new(segment: Segment) -> Self {
        Self {
            segment,
            slices: vec![],
            files: vec![],
        }
    }

    pub fn new_main() -> Self {
        let segment = Segment::Segment(SkewerCase::from_str("main").unwrap());
        Self {
            segment,
            slices: vec![],
            files: vec![],
        }
    }

    pub fn is_main(&self) -> bool {
        self.segment.is_main()
    }
    pub fn verify_children(&self) -> Result<(), PackageErr> {
        for slice in &self.slices {
            if slice.is_main() {
                return Err(PackageErr::MainSubSlice);
            }
            slice.verify_children()?;
        }

        Ok(())
    }
}

pub struct Directory {
    name: String,
    files: Vec<FileEntity>,
}

impl Directory {
    pub fn new(name: String) -> Self {
        Self {
            name,
            files: vec![],
        }
    }
}

pub enum Entity {
    File(FileEntity),
    Slice(Slice),
}
pub enum FileEntity {
    File(String),
    Directory(Directory),
}

#[cfg(test)]
mod tests {
    use super::Package;
}
