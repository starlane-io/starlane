use crate::create::{PackErr, PackageLayout};
use once_cell::sync::Lazy;
use starlane_space::parse::SkewerCase;
use starlane_space::types::scope::{Segment, SlicePath};
use std::collections::HashMap;
use std::fmt::Debug;
use std::io::Error;
use std::path::PathBuf;
use std::str::FromStr;
use std::{fs, io};
use thiserror::Error;

pub static MAIN_SLICE: Lazy<Segment> = Lazy::new(|| Segment::Segment(SkewerCase::from_str("main").unwrap()));

pub static PACKAGE_LAYOUT_EXAMPLE: Lazy<PathBuf> = Lazy::new(  || PathBuf::from_str("test/package-layout-example").unwrap());

pub mod create;

pub mod zip;
pub mod remote;
pub mod repo;
pub mod download;
pub mod server;

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


/// a [SliceLayout] is NOT a [Directory] but an independent part of a [PackageStructure] that
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
pub struct SliceLayout{
    /// the identity of this slice
    pub segment: Segment,
    slices: HashMap<Segment, SliceLayout>,
    directory: Directory,
}


impl SliceLayout {


    pub fn new(segment: Segment) -> Self {
        let name = segment.to_string();
        Self {
            segment,
            slices: Default::default(),
            directory: Directory::new(name)
        }
    }

    pub fn create(root: &PathBuf, observer: &mut dyn PackObserver) -> Result<Self, PackErr> {

        observer.start_pack( &root);

        observer.start_verify_layout();

        /// should only be called on a directory that is directly
        /// under a Slice (because it could be a sub-slice)
        fn walk_entity(dir: &PathBuf) -> Result<Entity, PackErr> {
            if crate::create::has_slice_file(dir) {
                crate::create::slice_name(dir)?;
                Ok(Entity::Slice(walk_slice(dir,None)?))
            } else {
                println!("DIR  : {}", dir.display());
                Ok(Entity::Directory(walk_dir(dir)?))
            }
        }

        /// `segment` is provided if its the Main segment
        fn walk_slice(dir: &PathBuf, segment: Option<Segment>) -> Result<SliceLayout, PackErr> {
            let name = match segment {
                None => crate::create::slice_name(&dir)?,
                Some(segment) => segment
            };

            let mut slice = SliceLayout::new(name);
            for entry in fs::read_dir(dir)? {
                let path = entry?.path();
                if path.is_file() {
                    if !crate::create::ignore(&path) {
                        let filename = crate::create::file_name(&path)?;
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

        /// walk a non-slice directory
        fn walk_dir(dir: &PathBuf) -> Result<Directory, PackErr> {
            let name = crate::create::file_name(dir)?;
            let mut directory = Directory::new(name);
            for entry in fs::read_dir(dir)? {
                let path = entry?.path();
                if !crate::create::ignore(&path) {
                    if path.is_file() {
                        let filename = crate::create::file_name(&path)?;
                        directory.children.insert(filename.clone(),FileEntity::File(filename));
                    } else {
                        let subdir = walk_dir(&path)?;
                        directory.children.insert(subdir.name.clone(),FileEntity::Directory(subdir));
                    }
                }
            }
            Ok(directory)
        }

        let mut main = walk_slice(root,Some(MAIN_SLICE.clone()))?;

        observer.end_pack();
        Ok(main)
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

    pub fn get_child_slice( &self, segment: &Segment) -> Option<&SliceLayout> {
        self.slices.get(segment)
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

    pub fn gather_slice_paths(&self) -> Vec<SlicePath> {
println!("GATHER SLIcE PATHS!");
        let mut paths = Vec::new();
        for slice in self.slices.values() {
            for mut p in slice.gather_slice_paths() {
                if !self.is_main() {
                    print!(" ---> p '{}'", p.to_string());
                    p.insert(self.segment.clone());

                    println!(" => '{}'", p.to_string());
                }
                paths.push(p);
            }
        }

        if !self.is_main() {
            let p : SlicePath = self.segment.clone().into();
println!("rtn  slice path: {}", p.to_string());
            paths.push(p);
        }
println!(" {} -[ paths ]-> ", paths.len() ) ;
        paths
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
    Slice(SliceLayout),
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
    use crate::create::PackageLayout;
    use crate::remote::RemoteRepo;
    use crate::repo::{Repo, SourceRepo};
    use crate::zip::unzip_from_binary_to_temp;
    use crate::{FileEntity, PackObserver, PublishObserver, PACKAGE_LAYOUT_EXAMPLE};
    use starlane_space::parse::SkewerCase;
    use starlane_space::types::scope::Segment;
    use std::fs;
    use std::str::FromStr;
    use tempfile::TempDir;

    pub struct MockPublishObserver();
    
    impl Default for MockPublishObserver {
        fn default() -> Self {
            Self()
        }
    }

    impl PublishObserver for MockPublishObserver {}
    impl PackObserver for MockPublishObserver {}

    #[tokio::test]
    pub async fn test_upload() {
        let mut observer = MockPublishObserver::default();
        let server = RemoteRepo::default();
        let layout = PackageLayout::create(&PACKAGE_LAYOUT_EXAMPLE, & mut observer).unwrap();
        server.submit(&layout, observer).await.unwrap();
    }

    fn verify_mock_layout(layout: &PackageLayout) -> Result<(),&'static str> {
        // hierarchy
        {
            // files
            let hierarchy_segment = Segment::Segment(SkewerCase::from_str("hierarchy").unwrap());
            let hierarchy = layout.slices.get(&hierarchy_segment).expect("expecting 'hierarchy'");
            assert!(hierarchy.directory.children.get(&"dir1".to_string()).expect("expecting 'dir1'").is_dir());
            assert!(!hierarchy.directory.children.get(&"little-file.txt".to_string()).expect("expecting 'dir1'").is_dir());
            assert_eq!(hierarchy.directory.children.len(),2);

            // slices
            assert_eq!(hierarchy.slices.len(),2);
        }


        // main
        {
            assert_eq!(layout.main.directory.children.len(),4);
            if let FileEntity::Directory(off) = layout.main.directory.children.get(&"off".to_string() ).expect("expecting 'off'") {
                assert_eq!(off.children.len(),2);
            } else {
                assert!(false)
            }
            assert_eq!(3,layout.main.slices.len());
        }
        Ok(())
    }

    #[test]
    pub fn test_create() {
        let mut observer = MockPublishObserver::default();
        let layout = PackageLayout::create(&PACKAGE_LAYOUT_EXAMPLE, & mut observer).unwrap();
        layout.diagnose();

        verify_mock_layout(&layout).unwrap();

        let repo_dir = TempDir::new().expect("expecting temp dir");
        let source = SourceRepo::new(repo_dir.path().to_path_buf());

        let zipfile = layout.zip().expect("expecting zip");
        let data= fs::read(zipfile.path()).expect("expecting zipfile");
        let unzip_dir= unzip_from_binary_to_temp(data.as_ref()).expect("unzipping bin zipfile");

        println!("\n\nzipfile: {:?}", zipfile);

        let path = unzip_dir.path().to_path_buf();
        let layout = PackageLayout::create(&path, & mut observer).unwrap();

        verify_mock_layout(&layout).unwrap();

        source.save_package(layout).unwrap();

        println!("\n\nzipfile: {:?}", zipfile);
    }
}

pub struct IgnoreObserver;

impl PackObserver for IgnoreObserver { }

pub trait PackObserver: Send + Sync{
    fn start_pack( &self, dir: &PathBuf ) {}
    fn start_verify_layout( &self ) {}

    fn found_slice(&self, name: &str) {}

    fn found_directory(&self, name: &str) {}
    fn found_file(&self, name: &str) {}
    fn end_verify_layout( &self, package: &PackageLayout) {}
    fn start_archive( &self ) {}
    fn end_archive( &self ) {}
    fn end_pack( &self ) {}
}

pub trait PublishObserver: PackObserver {
    fn start_upload( &self, server: &String ) {}
    fn end_upload( &self ) {}
}