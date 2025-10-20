use crate::zip::{zip_directory_to_temp, ZipError};
use crate::PackObserver;
use crate::SliceLayout;
use serde::de::DeserializeOwned;
use serde_derive::Deserialize;
use starlane_space::err::ParseErrs0;
use starlane_space::types::scope::Segment;
use starlane_space::types::specific::Release;
use std::ops::Deref;
use std::path::{PathBuf, StripPrefixError};
use std::str::FromStr;
use std::{fs, io};
use tempfile::NamedTempFile;
use thiserror::Error;

pub struct PackageLayout {
    pub config: PackageConfig,
    pub path: PathBuf,
    pub root: SliceLayout,
}

impl PackageLayout {
    pub fn release_directory(&self) -> PathBuf {
        let path = PathBuf::from(self.config.release.to_string().replace(":", "_"));
        path
    }

    pub fn release(&self) -> &Release {
        &self.config.release
    }

    pub fn create(path: &PathBuf, observer: &mut dyn PackObserver) -> Result<Self, PackErr> {
        let root = SliceLayout::create(path, observer)?;
        let toml_path = path.join("package.toml");
        let config: PackageConfigRaw = Self::read_toml(&toml_path)?;
        let config: PackageConfig = config.try_into()?;

        Ok(Self {
            config,
            path: path.clone(),
            root,
        })
    }

    pub fn get_slice(&self, segment: &str ) -> Option<&SliceLayout> {
            self.root.get_slice(segment)
    }

    fn read_toml<T: DeserializeOwned>(toml_path: &PathBuf) -> Result<T, PackErr> {
        let contents = fs::read_to_string(&toml_path)?;
        let parsed: T = toml::from_str(&contents)
            .map_err(|e| PackErr::TomlParseErr(toml_path.clone(), e.to_string()))?;
        Ok(parsed)
    }

    pub fn diagnose(&self) {
        self.diagnose_indent(0);
    }

    pub fn diagnose_indent(&self, mut spaces: usize) {
        let indent = " ".repeat(spaces);
        println!(
            "{indent}{}[PackageLayout] -> {}",
            self.path.display(),
            self.config.release.to_string()
        );
        self.root.diagnose_indent(spaces + 2);
    }

    pub fn zip(&self) -> Result<NamedTempFile, PackErr> {
        use crate::zip::zip_directory_to_temp;
        let path = zip_directory_to_temp(self.path.clone())?;
        Ok(path)
    }
}

#[derive(Debug, Deserialize)]
struct PackageConfigRaw {
    release: String,
}

struct PackageConfig {
    release: Release,
}

impl TryFrom<PackageConfigRaw> for PackageConfig {
    type Error = ParseErrs0;

    fn try_from(raw: PackageConfigRaw) -> Result<Self, Self::Error> {
        let specific = Release::from_str(raw.release.as_str())?;
        Ok(Self { release: specific })
    }
}

impl Deref for PackageLayout {
    type Target = SliceLayout;

    fn deref(&self) -> &Self::Target {
        &self.root
    }
}

#[derive(Debug, Error)]
pub enum PackErr {
    #[error("{0}")]
    SliceNameErr(ParseErrs0),
    #[error("slice not found: '{0}'")]
    SliceNotFound(String),
    #[error("{0}")]
    IoErr(std::io::Error),
    #[error("{0}")]
    StripPrefixErr(StripPrefixError),
    #[error("Invalid slice name: '{0}'")]
    InvalidSliceName(String),
    #[error("ZipErr: {0}")]
    ZipError(ZipError),
    #[error("TOML parse error in '{0}': {1}")]
    TomlParseErr(PathBuf, String),
}

impl From<ZipError> for PackErr {
    fn from(err: ZipError) -> Self {
        PackErr::ZipError(err)
    }
}

impl From<io::Error> for PackErr {
    fn from(err: io::Error) -> Self {
        PackErr::IoErr(err)
    }
}

impl From<ParseErrs0> for PackErr {
    fn from(errs: ParseErrs0) -> Self {
        PackErr::SliceNameErr(errs)
    }
}

impl From<StripPrefixError> for PackErr {
    fn from(errs: StripPrefixError) -> Self {
        PackErr::StripPrefixErr(errs)
    }
}

pub fn has_slice_file(path: &PathBuf) -> bool {
    if path.is_dir() {
        let slice_path = path.join(".slice");
        slice_path.is_file()
    } else {
        false
    }
}

pub fn slice_name(path: &PathBuf) -> Result<Segment, PackErr> {
    Ok(Segment::from_str(file_name(path)?.as_str())?)
}

pub fn file_name(path: &PathBuf) -> Result<String, PackErr> {
    Ok(path
        .file_name()
        .ok_or(PackErr::InvalidSliceName(format!("{}", path.display())))?
        .to_str()
        .ok_or(PackErr::InvalidSliceName(format!("{}", path.display())))?
        .to_string())
}

fn stringify(path: &PathBuf) -> Result<String, PackErr> {
    Ok(format!("{}", path.display()).to_string())
}

pub fn ignore(path: &PathBuf) -> bool {
    match file_name(path).unwrap().as_str() {
        ".slice" => true,
        _ => false,
    }
}
