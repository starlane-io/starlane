use crate::id::Id;
use tokio::sync::broadcast;
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
    pub id: Id,
    pub kind: ResourceKind
}

#[derive(Clone,Serialize,Deserialize)]
pub enum ResourceKind
{
    Single,
    Group
}



impl fmt::Display for ResourceKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({},{})", self.app_id, self.id)
    }
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ResourceLocation
{
    pub resource: ResourceKey,
    pub star: StarKey,
    pub group: Option<ResourceKey>,
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
            group: Option::None
        }
    }

    pub fn new_ext( resource: ResourceKey, star: StarKey, ext: Vec<u8>) -> Self
    {
        ResourceLocation{
            resource: resource,
            star: star,
            ext: Option::Some(ext),
            group: Option::None
        }
    }
}

pub struct ResourceGroup
{
    pub key: ResourceKey,
    pub resources: Vec<ResourceKey>
}


pub struct ResourceWatcher
{
    pub resource: ResourceKey,
    pub tx: Sender<ResourceEvent>
}

impl ResourceWatcher
{
    pub fn new( resource: ResourceKey )->(Self,broadcast::Receiver<ResourceEvent>)
    {
        let (tx,rx) = broadcast::channel(32);
        (ResourceWatcher{
            resource: resource,
            tx: tx
        },rx)
    }
}

impl ResourceWatcher
{
    pub fn notify( &self, event: ResourceEvent )
    {
        self.tx.send(event);
    }
}


