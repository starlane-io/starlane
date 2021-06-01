use crate::keys::{SubSpaceKey, ResourceKey};
use crate::resource::{ResourceAddress, Resource, ResourceType, StateSrc, State};
use crate::error::Error;

pub struct SubSpace{
    key: SubSpaceKey,
    address: ResourceAddress,
    name: String,
    display: String
}

impl SubSpace {
    pub fn new( key: SubSpaceKey, address: ResourceAddress, display: String )->Result<Self,Error>{
        if address.resource_type != ResourceType::SubSpace{
            Err("expected sub_space address".into())
        }
        else {
            Ok(SubSpace {
                key: key,
                name: address.last_to_string()?,
                display: display,
                address: address
            })
        }
    }
}

impl Resource<SubSpaceState> for SubSpace{
    fn key(&self) -> ResourceKey {
        ResourceKey::SubSpace(self.key.clone())
    }

    fn address(&self) -> ResourceAddress {
        self.address.clone()
    }

    fn resource_type(&self) -> ResourceType {
        ResourceType::SubSpace
    }

    fn state(&self) -> StateSrc<SubSpaceState> {
        StateSrc::Memory(SubSpaceState{
            name: self.name.clone(),
            display: self.display.clone()
        })
    }
}

#[derive(Clone,Serialize,Deserialize)]
pub struct SubSpaceState{
    name: String,
    display: String
}

impl SubSpaceState{
    pub fn from_bytes( bytes: &[u8] ) -> Result<Self,Error> {
        Ok(bincode::deserialize(bytes)?)
    }
}

impl State for SubSpaceState{
    fn to_bytes(self) -> Result<Vec<u8>, Error> {
        Ok(bincode::serialize(&self)?)
    }
}