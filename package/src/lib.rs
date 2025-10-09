use std::collections::HashMap;
use crate::create::PackErr;
use starlane_space::parse::SkewerCase;
use starlane_space::types::scope::Segment;
use std::fmt::Debug;
use std::io;
use std::io::Error;
use std::path::PathBuf;
use std::str::FromStr;
use thiserror::Error;
use once_cell::sync::Lazy;

pub static MAIN_SLICE: Lazy<Segment> = Lazy::new(|| Segment::Segment(SkewerCase::from_str("main").unwrap()));

pub static PACKAGE_LAYOUT_EXAMPLE: Lazy<PathBuf> = Lazy::new(  || PathBuf::from_str("test/package-layout-example").unwrap());

pub mod create;

pub mod zip;
pub mod server;

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
    #[error("{0}")]
    PackErr(PackErr),

   #[error("{0}")]
   RequestErr(reqwest::Error),

   #[error("UploadErr: {0}")]
   UploadErr(String),

   #[error("IOErr: {0}")]
   IOErr(io::Error)
}

impl From <io::Error> for PackageErr {
    fn from(value: Error) -> Self {
        PackageErr::IOErr(value)
    }
}

impl From <reqwest::Error> for PackageErr {
    fn from(err: reqwest::Error) -> Self {
        PackageErr::RequestErr(err)
    }
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
mod test {
    use std::path::PathBuf;
    use std::str::FromStr;
    use starlane_space::parse::SkewerCase;
    use starlane_space::types::scope::Segment;
    use crate::create::PackageDirectoryStructure;
    use crate::{FileEntity, PACKAGE_LAYOUT_EXAMPLE};
    use crate::server::PackageRepo;

    #[tokio::test]
    pub async fn test_upload() {
        let server = PackageRepo::default();
        let pds = PackageDirectoryStructure::create(&PACKAGE_LAYOUT_EXAMPLE).unwrap();
        server.upload_zip_file(&pds).await.unwrap();
    }


    #[test]
    pub fn test_create() {
        let pds= PackageDirectoryStructure::create(&PACKAGE_LAYOUT_EXAMPLE).unwrap();
        pds.diagnose();
        let structure = &pds.structure;

        // hierarchy
        {
            // files
            let hierarchy_segment = Segment::Segment(SkewerCase::from_str("hierarchy").unwrap());
            let hierarchy = structure.slices.get(&hierarchy_segment).expect("expecting 'hierarchy'");
            assert!(hierarchy.directory.children.get(&"dir1".to_string()).expect("expecting 'dir1'").is_dir());
            assert!(!hierarchy.directory.children.get(&"little-file.txt".to_string()).expect("expecting 'dir1'").is_dir());
            assert_eq!(hierarchy.directory.children.len(),2);

            // slices
            assert_eq!(hierarchy.slices.len(),2);
        }


        // main
        {
            let main = structure.main().expect("expecting 'main'");
            assert_eq!(main.directory.children.len(),3);
            if let FileEntity::Directory(off) = main.directory.children.get(&"off".to_string() ).expect("expecting 'off'") {
                assert_eq!(off.children.len(),2);
            } else {
                assert!(false)
            }
            assert!(main.slices.is_empty());
        }

        let zipfile = pds.zip().expect("expecting zip");

        println!("\n\nzipfile: {:?}", zipfile);
    }
}