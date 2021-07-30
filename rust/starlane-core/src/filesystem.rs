


use serde::{Deserialize, Serialize};

use crate::names::Name;

pub type FileSystem = Name;

#[derive(Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct File {
    pub filesystem: FileSystem,
    pub path: String,
}

pub struct FileData {
    pub file: File,
    pub data: Vec<u8>,
}
