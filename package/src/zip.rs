use std::fs::{self, File};
use std::io::{self, Error, Read, Write};
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

/// Zips a directory into a temporary file and returns the path to that file.
///
/// # Arguments
/// * `source_dir` - Path to the directory to zip
///
/// # Returns
/// * `Result<PathBuf, ZipError>` - Path to the temporary zip file on success
///
/// # Example
/// ```rust
/// use std::path::Path;
/// use starlane_package::zip::unzip_from_binary_to_temp;
///
/// let temp_zip = zip_directory_to_temp(Path::new("./my_folder"))?;
/// println!("Created zip at: {:?}", temp_zip);
/// ```
pub fn zip_directory_to_temp<P: AsRef<Path>>(source_dir: P) -> Result<NamedTempFile, ZipError> {
    let source_dir = source_dir.as_ref();

    // Validate that the source directory exists
    if !source_dir.exists() {
        return Err(ZipError::DirectoryNotFound(source_dir.to_path_buf()));
    }

    if !source_dir.is_dir() {
        return Err(ZipError::NotADirectory(source_dir.to_path_buf()));
    }

    // Create a temporary file with .zip extension
    let temp_file = NamedTempFile::new().map_err(ZipError::TempFileCreation)?;

    // Get the path before we move the temp file
    let temp_path = temp_file.path().to_path_buf();

    // Create the zip archive
    let file = temp_file.as_file();
    let mut zip = ZipWriter::new(file);

    // Set compression options
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o755);

    // Walk through the directory and add files to zip
    for entry in WalkDir::new(source_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        let name = path
            .strip_prefix(source_dir)
            .map_err(|_| ZipError::PathError(format!("Failed to strip prefix from {:?}", path)))?;

        // Skip the root directory itself
        if name.as_os_str().is_empty() {
            continue;
        }

        // Convert to string for zip entry name
        let name_str = name
            .to_str()
            .ok_or_else(|| ZipError::PathError(format!("Invalid UTF-8 in path: {:?}", name)))?;

        if path.is_file() {
            // Add file to zip
            zip.start_file(name_str, options)
                .map_err(ZipError::ZipOperation)?;

            let file_contents =
                fs::read(path).map_err(|e| ZipError::FileRead(path.to_path_buf(), e))?;

            zip.write_all(&file_contents)
                .map_err(|err| ZipError::WriteError(err))?;
        } else if path.is_dir() {
            // Add directory to zip (with trailing slash)
            let dir_name = format!("{}/", name_str);
            zip.add_directory(dir_name, options)
                .map_err(ZipError::ZipOperation)?;
        }
    }

    // Finalize the zip file
    zip.finish().map_err(ZipError::ZipOperation)?;

    Ok(temp_file)
}

/// Alternative version that allows custom temp directory
pub fn zip_slice_dir_to<P: AsRef<Path>, T: AsRef<Path>>(
    source_dir: P,
    target_file: T,
) -> Result<(), ZipError> {
    let source_dir = source_dir.as_ref();
    let target_file = target_file.as_ref();

    if !source_dir.exists() {
        return Err(ZipError::DirectoryNotFound(source_dir.to_path_buf()));
    }

    if !source_dir.is_dir() {
        return Err(ZipError::NotADirectory(source_dir.to_path_buf()));
    }

    let file = File::create(target_file)?;

    let mut zip = ZipWriter::new(file);

    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o755);

    for entry in WalkDir::new(source_dir)
        .into_iter()
        .filter_entry(|e| {
            if e.path() == source_dir {
                true
            } else
            // Skip directories that contain a .slice file
            if e.path().is_dir() {
                let slice_marker = e.path().join(".slice");
                let rtn = !slice_marker.exists();
                rtn
            } else {
                // Always include files
                true
            }
        })
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        let name = path
            .strip_prefix(source_dir)
            .map_err(|_| ZipError::PathError(format!("Failed to strip prefix from {:?}", path)))?;

        if name.as_os_str().is_empty() {
            continue;
        }

        let name_str = name
            .to_str()
            .ok_or_else(|| ZipError::PathError(format!("Invalid UTF-8 in path: {:?}", name)))?;

        if path.is_file() {
            zip.start_file(name_str, options)
                .map_err(ZipError::ZipOperation)?;

            let file_contents =
                fs::read(path).map_err(|e| ZipError::FileRead(path.to_path_buf(), e))?;

            zip.write_all(&file_contents)
                .map_err(ZipError::WriteError)?;
        } else if path.is_dir() {
            let dir_name = format!("{}/", name_str);
            zip.add_directory(dir_name, options)
                .map_err(ZipError::ZipOperation)?;
        }
    }

    zip.finish().map_err(ZipError::ZipOperation)?;
    Ok(())
}

/// Custom error type for zip operations
#[derive(Debug)]
pub enum ZipError {
    DirectoryNotFound(PathBuf),
    NotADirectory(PathBuf),
    TempFileCreation(io::Error),
    ZipOperation(zip::result::ZipError),
    FileRead(PathBuf, io::Error),
    PathError(String),
    WriteError(io::Error),
}

impl std::fmt::Display for ZipError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ZipError::DirectoryNotFound(path) => {
                write!(f, "Directory not found: {:?}", path)
            }
            ZipError::NotADirectory(path) => {
                write!(f, "Path is not a directory: {:?}", path)
            }
            ZipError::TempFileCreation(err) => {
                write!(f, "Failed to create temporary file: {}", err)
            }
            ZipError::ZipOperation(err) => {
                write!(f, "Zip operation failed: {}", err)
            }
            ZipError::FileRead(path, err) => {
                write!(f, "Failed to read file {:?}: {}", path, err)
            }
            ZipError::PathError(msg) => {
                write!(f, "Path error: {}", msg)
            }
            ZipError::WriteError(msg) => {
                write!(f, "Write Error: {}", msg)
            }
        }
    }
}

impl From<io::Error> for ZipError {
    fn from(err: Error) -> Self {
        Self::WriteError(err)
    }
}
impl std::error::Error for ZipError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_zip_directory_to_temp() {
        // Create a temporary directory with test files
        let temp_dir = TempDir::new().unwrap();
        let test_dir = temp_dir.path().join("test_folder");
        fs::create_dir(&test_dir).unwrap();

        // Create some test files
        fs::write(test_dir.join("file1.txt"), "Hello, World!").unwrap();
        fs::write(test_dir.join("file2.txt"), "This is a test").unwrap();

        // Create a subdirectory with a file
        let sub_dir = test_dir.join("subdir");
        fs::create_dir(&sub_dir).unwrap();
        fs::write(sub_dir.join("file3.txt"), "Nested file").unwrap();

        // Test the zip function
        let zip_path = zip_directory_to_temp(&test_dir).unwrap();
        let path = zip_path.path().to_path_buf();

        // Verify the zip file exists
        assert!(path.exists());
        assert!(path.metadata().unwrap().len() > 0);
    }

    #[test]
    fn test_nonexistent_directory() {
        let result = zip_directory_to_temp(Path::new("./nonexistent"));
        assert!(matches!(result, Err(ZipError::DirectoryNotFound(_))));
    }
}

// ... existing code ...

/// Unzips binary data to a temporary directory
///
/// # Arguments
/// * `zip_bytes` - Binary data of the zip file
///
/// # Returns
/// * `Result<tempfile::TempDir, ZipError>` - Temporary directory containing unzipped contents
///
/// # Example
/// ```rust
/// use starlane_package::zip::unzip_from_binary_to_temp;
/// let zip_data = std::fs::read("archive.zip").unwrap();
/// let temp_dir = unzip_from_binary_to_temp(&zip_data).unwrap();
/// println!("Unzipped to: {:?}", temp_dir.path());
/// ```
pub fn unzip_from_binary_to_temp(zip_bytes: &[u8]) -> Result<tempfile::TempDir, ZipError> {
    use std::io::Cursor;
    use zip::ZipArchive;

    // Create a temporary directory
    let temp_dir = tempfile::TempDir::new().map_err(ZipError::TempFileCreation)?;

    // Create a cursor from the bytes
    let cursor = Cursor::new(zip_bytes);

    // Open the zip archive
    let mut archive = ZipArchive::new(cursor).map_err(ZipError::ZipOperation)?;

    // Extract all files
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(ZipError::ZipOperation)?;
        let outpath = match file.enclosed_name() {
            Some(path) => temp_dir.path().join(path),
            None => continue,
        };

        if file.name().ends_with('/') {
            // It's a directory
            fs::create_dir_all(&outpath).map_err(|e| ZipError::FileRead(outpath.clone(), e))?;
        } else {
            // It's a file
            if let Some(parent) = outpath.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| ZipError::FileRead(parent.to_path_buf(), e))?;
            }

            let mut outfile =
                File::create(&outpath).map_err(|e| ZipError::FileRead(outpath.clone(), e))?;

            std::io::copy(&mut file, &mut outfile).map_err(ZipError::WriteError)?;
        }

        // Set permissions on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Some(mode) = file.unix_mode() {
                fs::set_permissions(&outpath, fs::Permissions::from_mode(mode))
                    .map_err(|e| ZipError::FileRead(outpath.clone(), e))?;
            }
        }
    }

    Ok(temp_dir)
}

pub fn unzip_from_file_to_temp(file: &PathBuf) -> Result<tempfile::TempDir, ZipError> {
    let mut content = vec![];
    let mut file = fs::File::open(file)?;
    file.read_to_end(&mut content)?;
    unzip_from_binary_to_temp(content.as_slice())
}

// ... existing code ...
/*
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Example 1: Basic usage
    let zip_path = zip_directory_to_temp("./src")?;
    println!("Created zip at: {:?}", zip_path);

    // Example 2: Custom temp directory
    let custom_temp = std::env::temp_dir();
    let zip_path2 = zip_directory_to_temp_in("./src", &custom_temp)?;
    println!("Created zip in custom location: {:?}", zip_path2);

    // Example 3: Persistent temp file
    let persistent_zip = zip_directory_to_persistent_temp("./src")?;
    println!("Created persistent zip: {:?}", persistent_zip);

    Ok(())
}
 */
