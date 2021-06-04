use crate::keys::{SubSpaceKey, ResourceKey};
use crate::resource::{ResourceAddress, Resource, ResourceType};
use crate::error::Error;
use serde::{Serialize,Deserialize};

pub struct SubSpace{
    key: SubSpaceKey,
    address: ResourceAddress,
}

