use std::convert::{TryFrom, TryInto};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use starlane_resources::Resource;

use crate::error::Error;
use crate::resource::{ResourceAddress, ResourceType, SubSpaceKey};

pub struct SubSpace {
    key: SubSpaceKey,
    address: ResourceAddress,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SubSpaceState {
    name: String,
}

impl SubSpaceState {
    pub fn new(name: &str) -> Self {
        SubSpaceState {
            name: name.to_string(),
        }
    }
}

impl TryInto<Vec<u8>> for SubSpaceState {
    type Error = Error;

    fn try_into(self) -> Result<Vec<u8>, Self::Error> {
        Ok(bincode::serialize(&self)?)
    }
}

impl TryInto<Arc<Vec<u8>>> for SubSpaceState {
    type Error = Error;

    fn try_into(self) -> Result<Arc<Vec<u8>>, Self::Error> {
        Ok(Arc::new(bincode::serialize(&self)?))
    }
}

impl TryFrom<Arc<Vec<u8>>> for SubSpaceState {
    type Error = Error;

    fn try_from(value: Arc<Vec<u8>>) -> Result<Self, Self::Error> {
        Ok(bincode::deserialize::<SubSpaceState>(value.as_slice())?)
    }
}

impl TryFrom<Vec<u8>> for SubSpaceState {
    type Error = Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        Ok(bincode::deserialize::<SubSpaceState>(value.as_slice())?)
    }
}
