use std::convert::{TryFrom, TryInto};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::resource::{AssignResourceStateSrc, LocalStateSetSrc, Resource, ResourceAddress, ResourceType, SrcTransfer, UserKey};

#[derive(Clone)]
pub struct User {
    key: UserKey,
    address: ResourceAddress,
    state_src: SrcTransfer<UserState>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct UserState {
    email: String,
}

impl UserState {
    pub fn new(email: String) -> Self {
        UserState { email: email }
    }
}

impl TryInto<Vec<u8>> for UserState {
    type Error = Error;

    fn try_into(self) -> Result<Vec<u8>, Self::Error> {
        Ok(bincode::serialize(&self)?)
    }
}

impl TryInto<Arc<Vec<u8>>> for UserState {
    type Error = Error;

    fn try_into(self) -> Result<Arc<Vec<u8>>, Self::Error> {
        Ok(Arc::new(bincode::serialize(&self)?))
    }
}

impl TryFrom<Arc<Vec<u8>>> for UserState {
    type Error = Error;

    fn try_from(value: Arc<Vec<u8>>) -> Result<Self, Self::Error> {
        Ok(bincode::deserialize::<UserState>(value.as_slice())?)
    }
}

impl TryFrom<Vec<u8>> for UserState {
    type Error = Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        Ok(bincode::deserialize::<UserState>(value.as_slice())?)
    }
}
