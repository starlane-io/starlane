use crate::id::Id;
use std::sync::Arc;
use crate::star::StarKey;
use crate::error::Error;
use crate::frame::{ResourceMessage, ResourceState, ResourceEvent};
use serde::{Deserialize, Serialize, Serializer};
use tokio::sync::broadcast::Sender;
use std::fmt;

#[derive(Eq,PartialEq,Hash,Clone,Serialize,Deserialize)]
pub struct ResourceKey
{
    pub app_id: Id,
    pub id: Id
}

impl fmt::Display for ResourceKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({},{})", self.app_id, self.id)
    }
}



#[derive(Eq,PartialEq,Hash,Clone,Serialize,Deserialize)]
pub struct ResourceGatheringKey
{
    pub app_id: Id,
    pub id: Id
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ResourceLocation
{
    pub resource: ResourceKey,
    pub star: StarKey,
    pub gathering: Option<ResourceGatheringKey>,
    pub ext: Option<Vec<u8>>
}

impl ResourceLocation
{
    pub fn new( resource: ResourceKey, star: StarKey )->Self
    {
        ResourceLocation{
            resource: resource,
            star: star,
            ext: Option::None,
            gathering: Option::None
        }
    }

    pub fn new_ext( resource: ResourceKey, star: StarKey, ext: Vec<u8>) -> Self
    {
        ResourceLocation{
            resource: resource,
            star: star,
            gathering: Option::None,
            ext: Option::Some(ext)
        }
    }
}

pub struct ResourceGathering
{
    pub key: ResourceGatheringKey,
    pub resources: Vec<ResourceKey>
}

pub enum ResourceWatchPattern
{
    Resource(ResourceKey),
    Gathering(ResourceGatheringKey)
}

pub struct ResourceWatcher
{
    pub pattern: ResourceWatchPattern,
    pub tx: Sender<ResourceEvent>
}

impl ResourceWatcher
{
    pub fn notify( &self, event: ResourceEvent )
    {
        self.tx.send(event);
    }
}


