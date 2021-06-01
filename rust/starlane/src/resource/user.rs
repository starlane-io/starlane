use crate::keys::{UserKey, ResourceKey};
use crate::resource::{State, Resource, ResourceAddress, ResourceType, StateSrc};
use crate::error::Error;

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
        StateSrc::Memory(UserState{
            email: email
        })
    }
}

#[derive(Clone,Serialize,Deserialize)]
pub struct UserState {
    pub email: String
}

impl UserState{
    pub fn from_bytes( bytes: &[u8] ) -> Result<Self,Error> {
        Ok(bincode::deserialize(bytes)?)
    }
}

impl State for UserState{
    fn to_bytes(self) -> Result<Vec<u8>, Error> {
        Ok(bincode::serialize(&self)?)
    }
}
