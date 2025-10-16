use std::fs;
use std::path::PathBuf;
use starlane_base::env::get_starlane_package_source;
use crate::create::{PackErr, PackageLayout};
use crate::zip::zip_slice_dir_to;
use starlane_space::types::scope::SlicePath;
use starlane_space::types::specific::{Release, Specific};

pub struct SourceRepo {
   root: PathBuf
}

impl Default for SourceRepo {
    fn default() -> Self {
        Self {
            root: PathBuf::from(get_starlane_package_source())
        }
    }
}

impl SourceRepo {
    pub fn new(path: PathBuf) -> Self {
        Self {
            root: path
        }
    }

    pub fn save_package( &self, package: PackageLayout ) -> Result<(),PackErr>{
println!("\nSAVE PACKAGE\n");
println!("root dir: {}", self.root.display());
        let release_dir = self.root.join(package.release_directory());
        fs::create_dir(release_dir.clone())?;
println!("release dir: {}", release_dir.display());
println!();
println!();
        /// first zip main/root which is a special case
        let main_target= release_dir.join(SlicePath::main().filename());
        zip_slice_dir_to(&package.root, main_target)?;

        let paths = package.gather_slice_paths();
        for p in paths {
            let source = package.root.join(p.as_path());
            let target = release_dir.join(p.filename());
            zip_slice_dir_to(source, target)?;
        }

        Ok(())
    }

    pub fn get_slice_path(&self, specific: Specific ) -> Result<PathBuf, String> {
        let release = specific.release().to_string().replace(":","_");
println!("release root: {}", release);
        let release_path = self.root.join(release);

        let slice_path = release_path.join(specific.slices().filename()).join(".zip");

        Ok(slice_path)
    }

}