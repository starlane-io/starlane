use crate::names::Name;
use serde::{Deserialize, Serialize};
use crate::keys::{SubSpaceKey, AppKey};
use std::sync::Arc;
use std::fmt;

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

impl fmt::Display for FileKey{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!( f,"[{},{},{}]",self.sub_space,self.filesystem,self.path )
    }

}

pub struct FileData
{
   pub file: File,
   pub data: Vec<u8>
}
