use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use serde::{Deserialize, Serialize, Serializer};
use tokio::sync::broadcast;
use tokio::sync::broadcast::Sender;

use crate::error::Error;
use crate::frame::{Event, ActorMessage, ActorState};
use crate::id::Id;
use crate::label::{LabelSelectionCriteria, Labels};
use crate::star::StarKey;
use crate::keys::{AppKey, UserKey, SubSpaceKey};
use crate::artifact::Name;
use crate::app::{InitData, ConfigSrc};
use crate::app::AppContext;

pub struct ActorContext
{
   pub meta: ActorMeta,
   pub app: AppContext
}

impl ActorContext
{
    pub fn new( meta: ActorMeta, app: AppContext )->Self
    {
        ActorContext{
            meta: meta,
            app: app
        }
    }
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ActorArchetype
{
    pub owner: UserKey,
    pub kind: ActorKind,
    pub specific: ActorSpecific,
    pub config: ConfigSrc,
    pub init: InitData,
    pub labels: Labels
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ActorMeta
{
    pub key: ActorKey,
    pub kind: ActorKind,
    pub config: ConfigSrc,
}

impl ActorMeta
{
    pub fn new( key: ActorKey, kind: ActorKind, config: ConfigSrc ) -> Self
    {
        ActorMeta{
            key: key,
            kind: kind,
            config: config
        }
    }
}




#[derive(Eq,PartialEq,Hash,Clone,Serialize,Deserialize)]
pub struct ActorInfo
{
    pub key: ActorKey,
    pub kind: ActorKind
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ActorProfile
{
    pub info: ActorInfo,
    pub labels: Labels,
}

#[derive(Eq,PartialEq,Hash,Clone,Serialize,Deserialize)]
pub struct ActorKey
{
    pub app: AppKey,
    pub id: Id,
}


#[derive(Clone)]
pub struct ActorRef
{
    pub key: ActorKey,
    pub archetype: ActorArchetype,
    pub actor: Arc<dyn Actor>
}

#[async_trait]
pub trait Actor: Sync+Send
{
    async fn handle_message(&mut self, actor_context: &ActorContext, message: ActorMessage );
}

pub type ActorSpecific = Name;
pub type GatheringSpecific = String;

#[derive(Eq,PartialEq,Hash,Clone,Serialize,Deserialize)]
pub enum ActorKind
{
    Actor(ActorSpecific),
    Gathering(GatheringSpecific)
}

impl ActorKind
{
    // it looks a little pointless but helps get around a compiler problem with static_lazy values
    pub fn as_kind(&self)->Self
    {
       self.clone()
    }
}
impl fmt::Display for ActorKind{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!( f,"{}",
                match self{
                    ActorKind::Actor(_) => "Actor".to_string(),
                    ActorKind::Gathering(_) => "Gathering".to_string()
                })
    }
}



impl fmt::Display for ActorKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({},{})", self.app, self.id)
    }
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ActorLocation
{
    pub actor: ActorKey,
    pub star: StarKey,
    pub gathering: Option<ActorKey>,
    pub ext: Option<Vec<u8>>
}

impl ActorLocation
{
    pub fn new(resource: ActorKey, star: StarKey ) ->Self
    {
        ActorLocation {
            actor: resource,
            star: star,
            ext: Option::None,
            gathering: Option::None
        }
    }

    pub fn new_ext(resource: ActorKey, star: StarKey, ext: Vec<u8>) -> Self
    {
        ActorLocation {
            actor: resource,
            star: star,
            ext: Option::Some(ext),
            gathering: Option::None
        }
    }
}

pub struct ActorGathering
{
    pub key: ActorKey,
    pub entity: Vec<ActorKey>
}


pub struct ActorWatcher
{
    pub entity: ActorKey,
    pub tx: Sender<Event>
}

impl ActorWatcher
{
    pub fn new(entity: ActorKey) ->(Self, broadcast::Receiver<Event>)
    {
        let (tx,rx) = broadcast::channel(32);
        (ActorWatcher {
            entity,
            tx: tx
        }, rx)
    }
}

impl ActorWatcher
{
    pub fn notify( &self, event: Event)
    {
        self.tx.send(event);
    }
}

#[derive(Clone,Serialize,Deserialize)]
pub struct MakeMeAnActor
{
    pub app: AppKey,
    pub kind: ActorKind,
    pub data: Arc<Vec<u8>>,
    pub labels: Labels
}

pub struct NewActor
{
    pub kind: ActorKind,
    pub data: Arc<Vec<u8>>,
    pub labels: Labels
}

pub struct ActorAssign
{
    pub key: ActorKey,
    pub kind: ActorKind,
    pub data: Arc<Vec<u8>>,
    pub labels: Labels
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ActorSelect
{
    criteria: Vec<LabelSelectionCriteria>
}

#[derive(Clone,Serialize,Deserialize)]
pub enum ActorStatus
{
    Unknown
}