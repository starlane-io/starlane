use std::fs::File;
use std::path::PathBuf;
use walkdir::WalkDir;
use zip::ZipArchive;

pub fn create(dir: &PathBuf) -> Result<ZipArchive<File>, std::io::Error> {
    walkdir::WalkDir::new(dir)
}

#[cfg(test)]
mod test
{

}