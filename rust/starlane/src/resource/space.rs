use crate::keys::{SpaceKey, ResourceKey};
use crate::resource::{Resource, ResourceAddress, ResourceType, State, StateSrc, ResourceSrc};
use crate::error::Error;
use std::convert::TryFrom;
use serde::{Serialize,Deserialize};

pub struct Space{
  key: SpaceKey,
  address: ResourceAddress,
  name: String,
  display: String
}

impl Space {

  pub fn new( key: SpaceKey, address: ResourceAddress, display: String )->Result<Self,Error>{
        if address.resource_type != ResourceType::Space{
            Err("expected space address".into())
        }
        else {
            Ok(Space {
                key: key,
                name: address.last_to_string()?,
                display: display,
                address: address
            })
        }
    }
}

impl Resource<SpaceState> for Space{
  fn key(&self) -> ResourceKey {
    ResourceKey::Space(self.key.clone())
  }

  fn address(&self) -> ResourceAddress {
      self.address.clone()
  }

  fn resource_type(&self) -> ResourceType {
      ResourceType::Space
  }

  fn state(&self) -> StateSrc<SpaceState> {
        StateSrc::Memory(Box::new(SpaceState{
            name: self.name.clone(),
            display: self.display.clone()
        }))
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

impl State for SpaceState{
  fn to_bytes(self) -> Result<Vec<u8>, Error> {
      Ok(bincode::serialize(&self)?)
  }
}

impl TryFrom<ResourceSrc> for StateSrc<SpaceState>{

    type Error = Error;

    fn try_from(src: ResourceSrc) -> Result<StateSrc<SpaceState>, Self::Error> {
        match src{
            ResourceSrc::AssignState(raw) => {
                Ok(StateSrc::Memory(Box::new(SpaceState::from_bytes(raw.as_slice())?)))
            }
        }
    }
}

