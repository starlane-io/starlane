use std::convert::{TryFrom, TryInto};
use std::sync::Arc;
use mesh_portal::version::latest::command::common::StateSrc;
use mesh_portal::version::latest::id::Point;

use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::particle::{
    KindBase,
};

#[derive(Clone)]
pub struct File {
    address: Point,
    state_src: StateSrc
}

#[derive(Clone, Serialize, Deserialize)]
pub struct FileState {
    content: Arc<Vec<u8>>,
}

impl FileState {
    pub fn new(content: Arc<Vec<u8>>) -> Self {
        FileState { content: content }
    }
}

impl TryInto<Vec<u8>> for FileState {
    type Error = Error;

    fn try_into(self) -> Result<Vec<u8>, Self::Error> {
        Ok(bincode::serialize(&self)?)
    }
}

impl TryInto<Arc<Vec<u8>>> for FileState {
    type Error = Error;

    fn try_into(self) -> Result<Arc<Vec<u8>>, Self::Error> {
        Ok(Arc::new(bincode::serialize(&self)?))
    }
}

impl TryFrom<Arc<Vec<u8>>> for FileState {
    type Error = Error;

    fn try_from(value: Arc<Vec<u8>>) -> Result<Self, Self::Error> {
        Ok(bincode::deserialize::<FileState>(value.as_slice())?)
    }
}

impl TryFrom<Vec<u8>> for FileState {
    type Error = Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        Ok(bincode::deserialize::<FileState>(value.as_slice())?)
    }
}
