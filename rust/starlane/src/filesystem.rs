use crate::names::Name;
use serde::{Deserialize, Serialize};
use crate::keys::SubSpaceKey;
use std::sync::Arc;

pub type FileSystem = Name;

#[derive(Clone,Eq,PartialEq,Hash,Serialize,Deserialize)]
pub struct File
{
    pub filesystem: FileSystem,
    pub path: String
}

#[derive(Clone,Eq,PartialEq,Hash,Serialize,Deserialize)]
pub struct FileKey
{
   pub sub_space: SubSpaceKey,
   pub filesystem: u64,
   pub path: u64
}

pub struct FileData
{
   pub file: File,
   pub data: Vec<u8>
}
