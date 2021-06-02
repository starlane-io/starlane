use crate::keys::{UserKey, ResourceKey};
use crate::resource::{State, Resource, ResourceAddress, ResourceType, StateSrc, ResourceSrc};
use crate::error::Error;
use serde::{Serialize,Deserialize};
use std::convert::TryFrom;

pub struct User{
    key: UserKey,
    email: String,
    address: ResourceAddress
}

impl Resource<UserState> for User{
    fn key(&self) -> ResourceKey {
        ResourceKey::User(self.key.clone())
    }

    fn address(&self) -> ResourceAddress {
        self.address.clone()
    }

    fn resource_type(&self) -> ResourceType {
        ResourceType::User
    }

    fn state(&self) -> StateSrc<UserState> {
        StateSrc::Memory(Box::new(UserState{
            email: self.email.clone()
        }))
    }
}

#[derive(Clone,Serialize,Deserialize)]
pub struct UserState {
    email: String,
}

impl UserState{
    pub fn from_bytes( bytes: &[u8] ) -> Result<Self,Error> {
        Ok(bincode::deserialize(bytes)?)
    }
}

impl UserState{

    pub fn new( email: String ) -> Self {
        UserState{
            email: email.clone(),
        }
    }

    pub fn email(&self)->String{
        self.email.clone()
    }

}


impl State for UserState{
    fn to_bytes(self) -> Result<Vec<u8>, Error> {
        Ok(bincode::serialize(&self)?)
    }
}

impl TryFrom<ResourceSrc> for StateSrc<UserState>{

    type Error = Error;

    fn try_from(src: ResourceSrc) -> Result<StateSrc<UserState>, Self::Error> {
        match src{
            ResourceSrc::AssignState(raw) => {
                Ok(StateSrc::Memory(Box::new(UserState::from_bytes(raw.as_slice())?)))
            }
        }
    }
}
