use crate::keys::{SpaceKey, ResourceKey};
use crate::resource::{Resource, ResourceAddress, ResourceType, State, StateSrc};
use crate::error::Error;

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
        StateSrc::Memory(SpaceState{
            name: self.name.clone(),
            display: self.display.clone()
        })
  }
}

#[derive(Clone,Serialize,Deserialize)]
pub struct SpaceState{
  name: String,
  display: String
}

impl SpaceState{
    pub fn from_bytes( bytes: &[u8] ) -> Result<Self,Error> {
        Ok(bincode::deserialize(bytes)?)
    }
}

impl State for SpaceState{
  fn to_bytes(self) -> Result<Vec<u8>, Error> {
      Ok(bincode::serialize(&self)?)
  }
}