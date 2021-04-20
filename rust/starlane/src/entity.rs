use crate::id::Id;
use tokio::sync::broadcast;
use std::sync::Arc;
use crate::star::StarKey;
use crate::error::Error;
use crate::frame::{ResourceMessage, ResourceState, EntityEvent};
use serde::{Deserialize, Serialize, Serializer};
use tokio::sync::broadcast::Sender;
use std::fmt;

#[derive(Eq,PartialEq,Hash,Clone,Serialize,Deserialize)]
pub struct EntityKey
{
    pub app_id: Id,
    pub id: Id,
    pub kind: EntityKind
}

#[derive(Eq,PartialEq,Hash,Clone,Serialize,Deserialize)]
pub enum EntityKind
{
    Single,
    Group
}



impl fmt::Display for EntityKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({},{})", self.app_id, self.id)
    }
}

#[derive(Clone,Serialize,Deserialize)]
pub struct EntityLocation
{
    pub entity: EntityKey,
    pub star: StarKey,
    pub group: Option<EntityKey>,
    pub ext: Option<Vec<u8>>
}

impl EntityLocation
{
    pub fn new(resource: EntityKey, star: StarKey ) ->Self
    {
        EntityLocation {
            entity: resource,
            star: star,
            ext: Option::None,
            group: Option::None
        }
    }

    pub fn new_ext(resource: EntityKey, star: StarKey, ext: Vec<u8>) -> Self
    {
        EntityLocation {
            entity: resource,
            star: star,
            ext: Option::Some(ext),
            group: Option::None
        }
    }
}

pub struct EntityGroup
{
    pub key: EntityKey,
    pub entity: Vec<EntityKey>
}


pub struct EntityWatcher
{
    pub entity: EntityKey,
    pub tx: Sender<EntityEvent>
}

impl EntityWatcher
{
    pub fn new(entity: EntityKey) ->(Self, broadcast::Receiver<EntityEvent>)
    {
        let (tx,rx) = broadcast::channel(32);
        (EntityWatcher {
            entity,
            tx: tx
        }, rx)
    }
}

impl EntityWatcher
{
    pub fn notify( &self, event: EntityEvent)
    {
        self.tx.send(event);
    }
}


