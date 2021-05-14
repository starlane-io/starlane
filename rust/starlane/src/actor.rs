use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

use serde::{Deserialize, Serialize, Serializer};
use tokio::sync::{broadcast, mpsc};
use tokio::sync::broadcast::Sender;

use crate::app::{ConfigSrc, InitData, AppFrom};
use crate::app::AppContext;
use crate::error::Error;
use crate::frame::{Event};
use crate::id::Id;
use crate::keys::{AppKey, ResourceKey, SubSpaceKey, UserKey};
use crate::names::Name;
use crate::resource::{Labels, Resource, ResourceKind, ResourceRegistration};
use crate::star::StarKey;
use std::marker::PhantomData;
use serde::de::DeserializeOwned;

pub struct Actor
{
    pub key: ActorKey,
    pub archetype: ActorArchetype
}

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
    pub labels: Labels,
    pub name: Option<String>,
}

impl ActorArchetype
{
  pub fn new( kind: ActorKind, specific: ActorSpecific, owner: UserKey )->Self
  {
      ActorArchetype{
          kind: kind,
          specific: specific,
          owner: owner,
          config: ConfigSrc::None,
          init: InitData::None,
          labels: Labels::new(),
          name: Option::None,
      }
  }
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ActorMeta
{
    pub key: ActorKey,
    pub kind: ActorKind,
    pub specific: ActorSpecific,
    pub config: ConfigSrc,
}

impl ActorMeta
{
    pub fn new( key: ActorKey, kind: ActorKind, specific: ActorSpecific, config: ConfigSrc ) -> Self
    {
        ActorMeta{
            key: key,
            kind: kind,
            specific: specific,
            config: config
        }
    }
}

pub struct ActorResource
{
    pub key: ActorKey,
    pub kind: ActorKind,
    pub specific: ActorSpecific,
    pub owner: UserKey
}

pub struct ActorRegistration
{
    pub resource: ActorResource,
    pub name: Option<String>,
    pub labels: Labels,
}

impl From<ActorResource> for Resource
{
    fn from(actor: ActorResource) -> Self {
        Resource{
            key: ResourceKey::Actor(actor.key),
            kind: ResourceKind::Actor(actor.kind),
            specific: Option::Some(actor.specific),
            owner: Option::Some(actor.owner)
        }
    }
}

impl From<ActorRegistration> for ResourceRegistration
{
    fn from(actor : ActorRegistration) -> Self {
        ResourceRegistration{
            resource: actor.resource.into(),
            name: actor.name,
            labels: actor.labels
        }
    }
}



#[derive(Eq,PartialEq,Hash,Clone,Serialize,Deserialize)]
pub struct ActorInfo
{
    pub key: ActorKey,
    pub kind: ActorKind
}

#[derive(Eq,PartialEq,Hash,Clone,Serialize,Deserialize)]
pub struct ActorKey
{
    pub app: AppKey,
    pub id: Id,
}

impl ActorKey
{
    pub fn new( app: AppKey, id: Id ) -> Self {
        ActorKey {
            app: app,
            id: id
        }
    }
}

pub type ActorSpecific = Name;
pub type GatheringSpecific = String;

#[derive(Eq,PartialEq,Hash,Clone,Serialize,Deserialize)]
pub enum ActorKind
{
    Single,
    Gathering
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
                    ActorKind::Single => "Single".to_string(),
                    ActorKind::Gathering => "Gathering".to_string()
                })
    }
}

impl FromStr for ActorKind
{
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s
        {
            "Single" => Ok(ActorKind::Single),
            "Gathering" => Ok(ActorKind::Gathering),
            _ => Err(format!("could not find ActorKind: {}",s).into())
        }
    }
}


impl fmt::Display for ActorKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({},{})", self.app, self.id)
    }
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ActorTo
{
    pub key: ActorKey,
    pub ext: ActorToExt

}

#[derive(Clone,Serialize,Deserialize)]
pub enum ActorToExt
{
    None,
    Ext(Raw)
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ActorFrom
{
    pub key: ActorKey,
    pub ext: ActorFromExt
}

#[derive(Clone,Serialize,Deserialize)]
pub enum ActorFromExt
{
    None,
    Ext(Raw)
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


pub struct ActorAssign
{
    pub key: ActorKey,
    pub kind: ActorKind,
    pub data: Arc<Vec<u8>>,
    pub labels: Labels
}


#[derive(Clone,Serialize,Deserialize)]
pub enum ActorStatus
{
    Unknown
}



#[derive(Clone)]
pub struct ActorKeySeq
{
    app: AppKey,
    seq: u64,
    index: u64,
    tx: mpsc::Sender<ActorKey>
}

impl ActorKeySeq
{
    pub fn new( app:AppKey, seq: u64, index: u64, tx: mpsc::Sender<ActorKey>)->Self {
        ActorKeySeq{
            app: app,
            seq: seq,
            index: index,
            tx: tx
        }
    }

    pub async fn next(&mut self)->ActorKey
    {
        self.index=self.index+1;
        let key = ActorKey::new(self.app.clone(), Id::new(self.seq, self.index ));

        self.tx.send(key.clone() ).await;

        key
    }
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ActorMessage
{
    pub id: Id,
    pub from: MessageFrom,
    pub to: ActorTo,
    pub payload: Arc<RawPayload>,
    pub transaction: Option<Id>
}


#[derive(Clone,Serialize,Deserialize)]
pub enum MessageFrom
{
    Actor(ActorFrom),
    App(AppFrom),
    User(UserKey)
}

pub type Raw=Vec<u8>;
pub type RawPayload=Vec<u8>;
pub type RawState=Vec<u8>;
