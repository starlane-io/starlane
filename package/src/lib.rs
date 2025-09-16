use std::collections::HashMap;
use crate::create::PackErr;
use starlane_space::parse::SkewerCase;
use starlane_space::types::scope::Segment;
use std::fmt::Debug;
use std::str::FromStr;
use thiserror::Error;
use once_cell::sync::Lazy;

pub static MAIN_SLICE: Lazy<Segment> = Lazy::new(|| Segment::Segment(SkewerCase::from_str("main").unwrap()));


#[cfg(feature = "create")]
pub mod create;

/// a convenience struct for understanding and
/// managing the anatomy of a package structure.

pub struct PackageStructure {
    slices: HashMap<Segment,Slice>,
}


impl PackageStructure {

    pub fn new( slices: HashMap<Segment,Slice> ) -> Self {
        Self { slices }
    }

    pub(crate) fn finalize(& mut self) {
        if !self.slices.contains_key(&MAIN_SLICE) {
            let main = Slice::new(MAIN_SLICE.clone());
            self.slices.insert(MAIN_SLICE.clone(), main);
        }
    }

    pub fn main(&self) -> Result<&Slice,PackageErr>{
        self.slices.get(&MAIN_SLICE).ok_or(PackageErr::MissingMainSlice)
    }

    pub fn diagnose(&self) {
        self.diagnose_indent(0);
    }

    pub fn diagnose_indent(&self,mut spaces:usize ) {
        let indent = " ".repeat(spaces);
        println!("{indent}[PackageStructure]");
        for (_,slice) in &self.slices {
            slice.diagnose_indent(spaces+2);
        }
    }


}

#[derive(Error, Debug)]
pub enum PackageErr {
    #[error("Main package slice must be named 'main'")]
    IllegalMain,
    #[error("subslices may not be named 'main'")]
    MainSubSlice,
   #[error("package missing 'main' slice")]
    MissingMainSlice,
    #[cfg(feature = "create")]
    #[error("{0}")]
    PackErr(PackErr),
}

impl From<PackErr> for PackageErr {
    fn from(err: PackErr) -> Self {
        PackageErr::PackErr(err)
    }
}

impl PackageStructure {
    pub fn verify(&self) -> Result<(), PackageErr> {
        Ok(())
    }
}

/// a [Slice] is NOT a [Directory] but an independent part of a [PackageStructure] that
/// can be downloaded separately and independently. For example imagine a package with slices:
/// ```md
/// * package `uberscott.com:website:1.2.3` with slices:
///   - frontend
///   - backend
///   - common
///   - cdn
/// ```
/// It behooves the developer to wrap all aspects of a release into one [PackageStructure] for consistency,
/// however, the `frontend` for example may not need to download the entire `backend` slice
/// to operate and likewise the `cdn` (Content Delivery Network) slice may be conveniently packaged
/// with the versions its meant to work with...
///
 #[derive(Clone,Debug)]
pub struct Slice {
    /// the identity of this slice
    segment: Segment,
    slices: HashMap<Segment,Slice>,
    directory: Directory,
}


impl Slice {


    pub fn new(segment: Segment) -> Self {
        let name = segment.to_string();
        Self {
            segment,
            slices: Default::default(),
            directory: Directory::new(name)
        }
    }

    pub fn new_main() -> Self {
        let name = "main";
        let segment = Segment::Segment(SkewerCase::from_str(name).unwrap());
        Self {
            segment,
            slices: Default::default(),
            directory: Directory::new(name.to_string()),
        }
    }

    pub fn is_main(&self) -> bool {
        self.segment.is_main()
    }
    pub fn verify_children(&self) -> Result<(), PackageErr> {
        for (_,slice) in &self.slices {
            if slice.is_main() {
                return Err(PackageErr::MainSubSlice);
            }
            slice.verify_children()?;
        }

        Ok(())
    }


    pub fn diagnose(&self) {
        self.diagnose_indent(0);
    }

    pub fn diagnose_indent(&self,mut spaces:usize ) {
        let indent = " ".repeat(spaces );

        println!("{indent}{}[Slice]",self.segment);
        self.directory.diagnose_indent(spaces+2,false );
        for (_,slice) in &self.slices {
            slice.diagnose_indent(spaces+2);
        }
    }
}

#[derive(Clone,Debug)]
pub struct Directory {
    name: String,
    children: HashMap<String,FileEntity>,
}

impl Directory {
    pub fn new(name: String) -> Self {
        Self {
            name,
            children: Default::default(),
        }
    }

    pub fn diagnose_indent(&self, mut spaces:usize, vanity: bool) {
        let indent = " ".repeat(spaces );

        if vanity {
            println!("{indent}{}[Directory]",self.name);
        }

        for (_,entry) in &self.children {
            match entry {
                FileEntity::File(file) => {
                    println!("{indent}..{}[File]",file);
                }
                _ => {}
            }
        }

        for (_,entry) in &self.children {
            match entry {
                FileEntity::Directory(directory) => {
                    directory.diagnose_indent(spaces+2, true);
                }
                _ => {}
            }
        }
    }
}

pub enum Entity {
    Directory(Directory),
    Slice(Slice),
}
#[derive(Clone,Debug)]
pub enum FileEntity {
    File(String),
    Directory(Directory),
}

impl FileEntity {
    pub fn is_dir(&self) -> bool{
        match self {
            Self::Directory(_) => true,
            _ => false,
        }
    }

}

#[cfg(test)]
mod tests {}
