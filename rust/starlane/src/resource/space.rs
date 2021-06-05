use crate::keys::{SpaceKey, ResourceKey};
use crate::resource::{Resource, ResourceAddress, ResourceType, AssignResourceStateSrc, LocalDataSrc, SrcTransfer};
use crate::error::Error;
use std::convert::{TryFrom, TryInto};
use serde::{Serialize,Deserialize};
use std::sync::Arc;

#[derive(Clone)]
pub struct Space{
    key: SpaceKey,
    address: ResourceAddress,
    state_src: SrcTransfer<SpaceState>
}

impl Space {

    pub fn new(key: SpaceKey, address: ResourceAddress, state_src: SrcTransfer<SpaceState>) ->Result<Self,Error>{
        if address.resource_type != ResourceType::Space{
            Err("expected space address".into())
        }
        else {
            Ok(Space {
                key: key,
                address: address,
                state_src: state_src
            })
        }
    }
}

#[derive(Clone,Serialize,Deserialize)]
pub struct SpaceState{
  name: String,
  display: String
}

impl SpaceState{

    pub fn new( name: &str, display: &str )-> Self{
        SpaceState{
            name: name.to_string(),
            display: display.to_string()
        }
    }

    pub fn name(&self)->String{
        self.name.clone()
    }

    pub fn display(&self)->String{
        self.display.clone()
    }

    pub fn from_bytes( bytes: &[u8] ) -> Result<Self,Error> {
        Ok(bincode::deserialize(bytes)?)
    }
}


impl TryInto<Vec<u8>> for SpaceState {

    type Error = Error;

    fn try_into(self) -> Result<Vec<u8>, Self::Error> {
        Ok(bincode::serialize(&self)?)
    }
}

impl TryInto<Arc<Vec<u8>>> for SpaceState {

    type Error = Error;

    fn try_into(self) -> Result<Arc<Vec<u8>>, Self::Error> {
        Ok(Arc::new(bincode::serialize(&self)?))
    }
}

impl TryFrom<Arc<Vec<u8>>> for SpaceState{
    type Error = Error;

    fn try_from(value: Arc<Vec<u8>>) -> Result<Self, Self::Error> {
        Ok(bincode::deserialize::<SpaceState>(value.as_slice() )?)
    }
}

impl TryFrom<Vec<u8>> for SpaceState{
    type Error = Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        Ok(bincode::deserialize::<SpaceState>(value.as_slice() )?)
    }
}