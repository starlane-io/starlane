#![allow(warnings)]
use crate::create::{PackErr, PackageLayout};
use once_cell::sync::Lazy;
use starlane_space::parse::SkewerCase;
use starlane_space::types::scope::{Segment, SlicePath};
use starlane_space::types::specific::{PackFile, Slice};
use std::collections::HashMap;
use std::fmt::Debug;
use std::io::Error;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::{fs, io};
use thiserror::Error;

pub static ROOT_SLICE: Lazy<Segment> =
    Lazy::new(|| Segment::Segment(SkewerCase::from_str("root").unwrap()));

pub static PACKAGE_LAYOUT_EXAMPLE: Lazy<PathBuf> =
    Lazy::new(|| PathBuf::from_str("test/package-layout-example").unwrap());

pub static PACKAGE: Lazy<Slice> =
    Lazy::new(|| Slice::from_str("uberscott.com:postgres:1.0.1").unwrap());
pub static MY_SLICE: Lazy<Slice> =
    Lazy::new(|| Slice::from_str("uberscott.com:postgres:1.0.1::my-slice").unwrap());
pub static ADVICE_FILE: Lazy<PackFile> =
    Lazy::new(|| PackFile::from_str("uberscott.com:postgres:1.0.1::my-slice/advice.txt").unwrap());

pub mod create;

pub mod download;
pub mod remote;
pub mod repo;
pub mod server;
pub mod zip;
mod cache;

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
    IOErr(io::Error),
}

impl From<io::Error> for PackageErr {
    fn from(value: Error) -> Self {
        PackageErr::IOErr(value)
    }
}

impl From<reqwest::Error> for PackageErr {
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
#[derive(Clone, Debug)]
pub struct SliceLayout {
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
            directory: Directory::new(name),
        }
    }

    /// return true if this path is in the directory structure
    /// false if it doesn't exist or is a slice
    pub fn is_member(&self, path: &PathBuf) -> bool {
        self.directory.is_member(path)
    }

    pub fn get_slice(&self, segment: &str) -> Option<&SliceLayout> {
        if let Ok(segment) = Segment::from_str(segment) {
            self.slices.get(&segment)
        } else {
            None
        }
    }

    pub fn create(root: &PathBuf, observer: &dyn PackObserver) -> Result<Self, PackErr> {
        observer.start_pack(&root);

        observer.start_verify_layout();

        /// should only be called on a directory that is directly
        /// under a Slice (because it could be a sub-slice)
        fn walk_entity(dir: &PathBuf) -> Result<Entity, PackErr> {
            if crate::create::has_slice_file(dir) {
                crate::create::slice_name(dir)?;
                Ok(Entity::Slice(walk_slice(dir, None)?))
            } else {
                Ok(Entity::Directory(walk_dir(dir)?))
            }
        }

        /// `segment` is provided if its the Main segment
        fn walk_slice(dir: &PathBuf, segment: Option<Segment>) -> Result<SliceLayout, PackErr> {
            let name = match segment {
                None => crate::create::slice_name(&dir)?,
                Some(segment) => segment,
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
                            slice
                                .directory
                                .children
                                .insert(directory.name.clone(), FileEntity::Directory(directory));
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
                        directory
                            .children
                            .insert(filename.clone(), FileEntity::File(filename));
                    } else {
                        let subdir = walk_dir(&path)?;
                        directory
                            .children
                            .insert(subdir.name.clone(), FileEntity::Directory(subdir));
                    }
                }
            }
            Ok(directory)
        }

        let mut main = walk_slice(root, Some(ROOT_SLICE.clone()))?;

        observer.end_pack();
        Ok(main)
    }

    pub fn new_root() -> Self {
        let name = "root";
        let segment = Segment::Segment(SkewerCase::from_str(name).unwrap());
        Self {
            segment,
            slices: Default::default(),
            directory: Directory::new(name.to_string()),
        }
    }

    pub fn is_main(&self) -> bool {
        self.segment.is_root()
    }
    pub fn verify_children(&self) -> Result<(), PackageErr> {
        for (_, slice) in &self.slices {
            if slice.is_main() {
                return Err(PackageErr::MainSubSlice);
            }
            slice.verify_children()?;
        }

        Ok(())
    }

    pub fn get_child_slice(&self, segment: &Segment) -> Option<&SliceLayout> {
        self.slices.get(segment)
    }

    pub fn diagnose(&self) {
        self.diagnose_indent(0);
    }

    pub fn diagnose_indent(&self, mut spaces: usize) {
        let indent = " ".repeat(spaces);

        println!("{indent}{}[Slice]", self.segment);
        self.directory.diagnose_indent(spaces + 2, false);
        for (_, slice) in &self.slices {
            slice.diagnose_indent(spaces + 2);
        }
    }

    pub fn gather_slice_paths(&self) -> Vec<SlicePath> {
        let mut paths = Vec::new();
        for slice in self.slices.values() {
            for mut p in slice.gather_slice_paths() {
                if !self.is_main() {
                    p.insert(self.segment.clone());
                }
                paths.push(p);
            }
        }

        if !self.is_main() {
            let p: SlicePath = self.segment.clone().into();
            paths.push(p);
        }
        paths
    }
}

#[derive(Clone, Debug)]
pub struct Directory {
    name: String,
    children: HashMap<String, FileEntity>,
}

impl Directory {
    pub fn new(name: String) -> Self {
        Self {
            name,
            children: Default::default(),
        }
    }

    pub fn is_member(&self, path: &Path) -> bool {
        if path.is_absolute() {
            return false;
        }

        if let Some(first) = first_component(path) {
            if let Some(child) = self.children.get(&first) {
                match child {
                    FileEntity::File(file) => {
                        if path.components().count() == 1 {
                            file == &first
                        } else {
                            false
                        }
                    }
                    FileEntity::Directory(directory) => {
                        if path.components().count() == 1 {
                            directory.name == first
                        } else {
                            let path = path.strip_prefix(&first).unwrap();
                            directory.is_member(path)
                        }
                    }
                }
            } else {
                false
            }
        } else {
            false
        }
    }

    pub fn diagnose_indent(&self, mut spaces: usize, vanity: bool) {
        let indent = " ".repeat(spaces);

        if vanity {
            println!("{indent}{}[Directory]", self.name);
        }

        for (_, entry) in &self.children {
            match entry {
                FileEntity::File(file) => {
                    println!("{indent}..{}[File]", file);
                }
                _ => {}
            }
        }

        for (_, entry) in &self.children {
            match entry {
                FileEntity::Directory(directory) => {
                    directory.diagnose_indent(spaces + 2, true);
                }
                _ => {}
            }
        }
    }
}
pub fn first_component(path: &std::path::Path) -> Option<String> {
    path.components().find_map(|comp| match comp {
        std::path::Component::Normal(os_str) => os_str.to_str().map(|s| s.to_string()),
        _ => None,
    })
}
pub enum Entity {
    Directory(Directory),
    Slice(SliceLayout),
}
#[derive(Clone, Debug)]
pub enum FileEntity {
    File(String),
    Directory(Directory),
}

impl FileEntity {
    pub fn is_dir(&self) -> bool {
        match self {
            Self::Directory(_) => true,
            _ => false,
        }
    }
}


/// this fella basically ignores all events
struct IgnorantPublishObserver;

impl Default for IgnorantPublishObserver {
    fn default() -> Self {
        Self
    }
}

impl IgnorantPublishObserver {
    pub fn new() -> Box<Self> {
        Box::new(Self)
    }
}

impl PublishObserver for IgnorantPublishObserver {}
impl PackObserver for IgnorantPublishObserver {}


#[cfg(test)]
mod test {
    use crate::create::PackageLayout;
    use crate::repo::{Repo, SourceRepo};
    use crate::zip::{unzip_from_binary_to_temp, unzip_from_file_to_temp, zip_slice_dir_to};
    use crate::{IgnorantPublishObserver, FileEntity, PackObserver, PublishObserver, MY_SLICE, PACKAGE, PACKAGE_LAYOUT_EXAMPLE, new_ignorant_observer};
    use starlane_space::parse::SkewerCase;
    use starlane_space::types::scope::Segment;
    use std::fs;
    use std::path::Path;
    use std::str::FromStr;
    use tempfile::NamedTempFile;
    use tokio::io::AsyncWriteExt;

    #[cfg(test)]
    mod server {
        use crate::cache::PackageCache;
        use crate::create::PackageLayout;
        use crate::download::Downloader;
        use crate::remote::{RemoteRepo, Repo};
        use crate::server::{ServerBuilder, ServerControl};
        use crate::zip::unzip_from_binary_to_temp;
        use crate::{new_ignorant_observer, IgnorantPublishObserver, PackObserver, ADVICE_FILE, MY_SLICE, PACKAGE_LAYOUT_EXAMPLE};
        use tokio::io::AsyncWriteExt;

        pub struct RemoteTest {
            pub server_control: ServerControl,
            pub remote_repo: RemoteRepo
        }

        impl RemoteTest {
            pub async fn mock() -> Self {
                let server_control = ServerBuilder::mock().await;
                let port = server_control.get_port().await.unwrap();
                println!("Server started");
                let remote_repo = RemoteRepo::local_with_port(port);

                Self {
                    server_control,
                    remote_repo
                }
            }
        }

        #[tokio::test]
        pub async fn test_downloader() {
            let test = RemoteTest::mock().await;
            let downloader = Downloader::temp(test.remote_repo.clone());
            downloader.download(&MY_SLICE).await.unwrap();
        }

        #[tokio::test]
        pub async fn test_upload_and_download() {
            let test = RemoteTest::mock().await;
            let observer: Box<dyn PackObserver> = new_ignorant_observer();
            let layout = PackageLayout::create(&PACKAGE_LAYOUT_EXAMPLE, &*observer).unwrap();
            test.remote_repo.publish(&layout).await.unwrap();

            let my_slice = test.remote_repo.get_slice(&MY_SLICE).await.unwrap();

            let slice_dir = unzip_from_binary_to_temp(my_slice.as_slice()).unwrap();
            let slice_path = slice_dir.path().to_path_buf();
            let advice = slice_path.join("advice.txt");
            let mut stdout = tokio::io::stdout();
            stdout.flush().await.unwrap();
            assert!(advice.exists());
        }


        #[tokio::test]
        pub async fn test_cache() {
            let test = RemoteTest::mock().await;
            let cache = PackageCache::unique_with_keep(test.remote_repo.clone(), true );
            cache.get_file(&ADVICE_FILE).await.unwrap();
        }

    }


    pub fn package_layout() -> PackageLayout {
        let observer = new_ignorant_observer();
        PackageLayout::create(&PACKAGE_LAYOUT_EXAMPLE, &*observer).unwrap()
    }

    #[test]
    pub fn test_slice_membership() {
        let package = package_layout();
        let hierarchy = package.get_slice("hierarchy").unwrap();
        let path = Path::new("dir1").to_path_buf();
        assert!(hierarchy.is_member(&path));
        let path = Path::new("sub1").to_path_buf();
        assert!(!hierarchy.is_member(&path));
    }

    #[tokio::test]
    pub async fn test_zip_slice() {
        let in_dir = PACKAGE_LAYOUT_EXAMPLE.join("hierarchy");
        let layout = package_layout();
        let hierarchy = layout.get_slice("hierarchy").unwrap();
        let mut tmp_file = NamedTempFile::new().unwrap();
        let out_file = tmp_file.path().to_path_buf();
        zip_slice_dir_to(&in_dir, &out_file).unwrap();
        let unzip_temp = unzip_from_file_to_temp(&out_file).unwrap();
        let tmp_dir = unzip_temp.path().to_path_buf();
        let dir1 = tmp_dir.join("dir1");
        let sub1 = tmp_dir.join("sub1");
        let sub2 = tmp_dir.join("sub2");

        assert!(dir1.exists());
        assert!(!sub1.exists());
        assert!(!sub2.exists());
    }

    /*
    #[tokio::test]
    pub async fn test_source() {
        let (source, _dir) = SourceRepo::temp();
        let layout = package_layout();
        source.publish(layout).unwrap();
        {
            let zip = source.get_slice(&MY_SLICE).await.unwrap();
            let dir = unzip_from_binary_to_temp(zip.as_slice()).unwrap();
            let path = dir.path().to_path_buf().join("advice.txt");
            assert!(path.exists())
        }

        {
            let zip = source.get_slice(&PACKAGE).await.unwrap();
            let dir = unzip_from_binary_to_temp(zip.as_slice()).unwrap();
            let path = dir.path().to_path_buf().join("some-file.txt");
            assert!(path.exists())
        }
    }

     */


     /// test if local calls from a [SourceRepo] created via [SourceRepo::mock] will deliver the
     /// slices in the proper zip format and spot checks for certain files in those slices.
     ///
     /// This test is run locally without a network server mechanism.
     #[tokio::test]
    pub async fn test_mock_source() {
        let repo = SourceRepo::mock().await;

        {
            let zip = repo.get_slice(&MY_SLICE).await.unwrap();
            let dir = unzip_from_binary_to_temp(zip.as_slice()).unwrap();
            let path = dir.path().to_path_buf().join("advice.txt");
            assert!(path.exists())
        }

        {
            let zip = repo.get_slice(&PACKAGE).await.unwrap();
            let dir = unzip_from_binary_to_temp(zip.as_slice()).unwrap();
            let path = dir.path().to_path_buf().join("some-file.txt");
            assert!(path.exists())
        }

        {
            let zip = repo.get_slice(&MY_SLICE).await.unwrap();
            let dir = unzip_from_binary_to_temp(zip.as_slice()).unwrap();
            let path = dir.path().to_path_buf().join("this-file-should-not-exist.txt");
            assert!(!path.exists())
        }

    }










    fn verify_mock_layout(layout: &PackageLayout) -> Result<(), &'static str> {
        // hierarchy
        {
            // files
            let hierarchy_segment = Segment::Segment(SkewerCase::from_str("hierarchy").unwrap());
            let hierarchy = layout
                .slices
                .get(&hierarchy_segment)
                .expect("expecting 'hierarchy'");
            assert!(hierarchy
                .directory
                .children
                .get(&"dir1".to_string())
                .expect("expecting 'dir1'")
                .is_dir());
            assert!(!hierarchy
                .directory
                .children
                .get(&"little-file.txt".to_string())
                .expect("expecting 'dir1'")
                .is_dir());
            assert_eq!(hierarchy.directory.children.len(), 2);

            // slices
            assert_eq!(hierarchy.slices.len(), 2);
        }

        // main
        {
            assert_eq!(layout.root.directory.children.len(), 4);
            if let FileEntity::Directory(off) = layout
                .root
                .directory
                .children
                .get(&"off".to_string())
                .expect("expecting 'off'")
            {
                assert_eq!(off.children.len(), 2);
            } else {
                assert!(false)
            }
            assert_eq!(3, layout.root.slices.len());
        }
        Ok(())
    }

    #[tokio::test]
    pub async fn test_publish() {
        let mut observer = IgnorantPublishObserver::default();
        let layout = PackageLayout::create(&PACKAGE_LAYOUT_EXAMPLE, &mut observer).unwrap();
        layout.diagnose();

        verify_mock_layout(&layout).unwrap();

        let repo = SourceRepo::temp();

        let zipfile = layout.zip().expect("expecting zip");
        let data = fs::read(zipfile.path()).expect("expecting zipfile");
        let unzip_dir = unzip_from_binary_to_temp(data.as_ref()).expect("unzipping bin zipfile");

        println!("\n\nzipfile: {:?}", zipfile);

        let path = unzip_dir.path().to_path_buf();
        let layout = PackageLayout::create(&path, &mut observer).unwrap();

        verify_mock_layout(&layout).unwrap();

        repo.publish(&layout, ).await.unwrap();

        println!("\n\nzipfile: {:?}", zipfile);
    }
}

pub struct IgnoreObserver;

impl PackObserver for IgnoreObserver {}

pub trait PackObserver: Send + Sync {
    fn start_pack(&self, dir: &PathBuf) {}
    fn start_verify_layout(&self) {}

    fn found_slice(&self, name: &str) {}

    fn found_directory(&self, name: &str) {}
    fn found_file(&self, name: &str) {}
    fn end_verify_layout(&self, package: &PackageLayout) {}
    fn start_archive(&self) {}
    fn end_archive(&self) {}
    fn end_pack(&self) {}
}

pub trait PublishObserver: PackObserver {
    fn start_upload(&self, server: &String) {}
    fn end_upload(&self) {}
}

fn new_ignorant_observer() -> Box<dyn PublishObserver> {
    IgnorantPublishObserver::new()
}
