use crate::keys::{FileSystemKey, ResourceKey};
use crate::resource::{Resource, ResourceAddress, ResourceType, AssignResourceStateSrc, LocalDataSrc, SrcTransfer};
use crate::error::Error;
use serde::{Serialize,Deserialize};
use std::convert::{TryFrom, TryInto};
use std::sync::Arc;

#[derive(Clone)]
pub struct FileSystem{
    key: FileSystemKey,
    address: ResourceAddress,
    state_src: SrcTransfer<FileSystemState>
}

#[derive(Clone,Serialize,Deserialize)]
pub struct FileSystemState {
}

impl FileSystemState {
    pub fn new( ) -> Self {
        FileSystemState {}
    }
}

impl TryInto<Vec<u8>> for FileSystemState {

    type Error = Error;

    fn try_into(self) -> Result<Vec<u8>, Self::Error> {
        Ok(bincode::serialize(&self)?)
    }
}

impl TryInto<Arc<Vec<u8>>> for FileSystemState {

    type Error = Error;

    fn try_into(self) -> Result<Arc<Vec<u8>>, Self::Error> {
        Ok(Arc::new(bincode::serialize(&self)?))
    }
}

impl TryFrom<Arc<Vec<u8>>> for FileSystemState{
    type Error = Error;

    fn try_from(value: Arc<Vec<u8>>) -> Result<Self, Self::Error> {
        Ok(bincode::deserialize::<FileSystemState>(value.as_slice() )?)
    }
}

impl TryFrom<Vec<u8>> for FileSystemState{
    type Error = Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        Ok(bincode::deserialize::<FileSystemState>(value.as_slice() )?)
    }
}