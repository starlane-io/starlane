use crate::keys::DomainKey;
use crate::resource::ResourceAddress;
use std::convert::{TryInto, TryFrom};
use std::sync::Arc;
use crate::error::Error;
use serde::{Serialize,Deserialize};

pub struct Domain{
    key: DomainKey,
    address: ResourceAddress,
}

#[derive(Clone,Serialize,Deserialize)]
pub struct DomainState {
}


impl DomainState{
    pub fn new() -> Self {
        DomainState{
        }
    }
}

impl TryInto<Vec<u8>> for DomainState {

    type Error = Error;

    fn try_into(self) -> Result<Vec<u8>, Self::Error> {
        Ok(bincode::serialize(&self)?)
    }
}

impl TryInto<Arc<Vec<u8>>> for DomainState {

    type Error = Error;

    fn try_into(self) -> Result<Arc<Vec<u8>>, Self::Error> {
        Ok(Arc::new(bincode::serialize(&self)?))
    }
}

impl TryFrom<Arc<Vec<u8>>> for DomainState{
    type Error = Error;

    fn try_from(value: Arc<Vec<u8>>) -> Result<Self, Self::Error> {
        Ok(bincode::deserialize::<DomainState>(value.as_slice() )?)
    }
}

impl TryFrom<Vec<u8>> for DomainState{
    type Error = Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        Ok(bincode::deserialize::<DomainState>(value.as_slice() )?)
    }
}