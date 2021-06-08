use std::collections::{HashMap, HashSet};
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

use serde::{Deserialize, Serialize, Serializer};
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::sync::broadcast::Sender;

use crate::app::{ConfigSrc, InitData};
use crate::app::AppContext;
use crate::error::Error;
use crate::frame::{Event};
use crate::id::Id;
use crate::keys::{AppKey, ResourceKey, SubSpaceKey, UserKey, ResourceId};
use crate::names::Name;
use crate::resource::{Labels, ResourceAssign, ResourceKind, ResourceRegistration, ResourceType, ResourceArchetype, ResourceInit, Names, ResourceAddress, ResourceAddressPart, SkewerCase, ResourceRegistryInfo, ResourceStub, ResourceCreate, ResourceSelector, ResourceRecord};
use crate::star::StarKey;
use std::marker::PhantomData;
use serde::de::DeserializeOwned;
use crate::message::Fail;
use crate::resource::ResourceAddressPartKind::Base64Encoded;
use std::iter::FromIterator;

pub struct Actor
{
    pub key: ResourceKey,
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
pub struct ActorProfile{
    pub archetype: ActorArchetype,
    pub init: InitData
}

impl From<ActorProfile> for ResourceInit
{
    fn from(profile: ActorProfile) -> Self {
        ResourceInit {
            init: profile.init,
            archetype: profile.archetype.into(),
        }
    }
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ActorArchetype
{
    pub kind: ActorKind,
    pub specific: ActorSpecific,
    pub config: ConfigSrc,
}

impl From<ActorArchetype> for ResourceArchetype
{
    fn from(archetype: ActorArchetype) -> Self {
        ResourceArchetype{
            kind: ResourceKind::Actor(archetype.kind),
            specific: Option::Some(archetype.specific),
            config: Option::Some(archetype.config)
        }
    }
}


impl ActorArchetype {
    pub fn resource_archetype(self)->ResourceArchetype{
        ResourceArchetype{
            kind: ResourceKind::Actor(self.kind),
            specific: Option::Some(self.specific),
            config: Option::Some(self.config)
        }
    }
}


impl ActorArchetype
{
  pub fn new( kind: ActorKind, specific: ActorSpecific, config: ConfigSrc )->Self
  {
      ActorArchetype{
          kind: kind,
          specific: specific,
          config: config
      }
  }
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ActorMeta
{
    pub key: ResourceKey,
    pub kind: ActorKind,
    pub specific: ActorSpecific,
    pub config: ConfigSrc,
}

impl ActorMeta
{
    pub fn new(key: ResourceKey, kind: ActorKind, specific: ActorSpecific, config: ConfigSrc ) -> Self
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
    pub owner: UserKey,
    pub archetype: ActorArchetype,
    pub address: ResourceAddress
}

pub struct ActorRegistration
{
    pub resource: ActorResource,
    pub names: Names,
    pub labels: Labels
}

impl From<ActorResource> for ResourceStub
{
    fn from(actor: ActorResource) -> Self {
        ResourceStub{
            key: ResourceKey::Actor(actor.key),
            archetype: ResourceArchetype{
                kind: ResourceKind::Actor(actor.archetype.kind),
                specific: Option::Some(actor.archetype.specific),
                config: Option::Some(actor.archetype.config)
            },
            owner: Option::Some(actor.owner),
            address: actor.address
        }
    }
}

/*
impl From<ActorRegistration> for ResourceRegistration
{
    fn from(actor : ActorRegistration) -> Self {
        ResourceRegistration{
            resource: actor.resource.into(),
            location: ResourceLocation {},
            info: Option::Some(ResourceRegistryInfo {
                names: actor.names,
                labels: actor.labels
            })
        }
    }
}
 */



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

impl ActorKey{
    pub fn address_part(&self) -> Result<ResourceAddressPart,Error>{
        Ok(ResourceAddressPart::SkewerCase(SkewerCase::new(self.id.to_string().as_str() )?))
    }
}


impl ActorKey
{
    pub fn new( app: AppKey, id: Id ) -> Self {
        ActorKey{
            app: app,
            id: id
        }
    }
}

impl ToString for ActorKey{
    fn to_string(&self) -> String {
        format!("{}-{}",self.app.to_string(), self.id.to_string())
    }
}

impl FromStr for ActorKey{
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let pos = s.rfind( '-').ok_or("expected '-' between parent and id")?;
        let (parent,id)= s.split_at(pos);
        let app= AppKey::from_str(parent)?;
        let id = Id::from_str(id)?;
        Ok(ActorKey{
            app: app,
            id: id
        })
    }
}

pub type ActorSpecific = Name;
pub type GatheringSpecific = Name;

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



#[derive(Clone,Serialize,Deserialize)]
pub enum ResourceToExt
{
    None,
    Ext(Raw)
}

#[derive(Clone,Serialize,Deserialize)]
pub enum ResourceFrom
{
    Injected
}

#[derive(Clone,Serialize,Deserialize)]
pub enum ResourceFromExt
{
    None,
    Ext(Raw)
}

impl ResourceFromExt
{
    pub fn reverse(&self)->ResourceToExt {
        match self {
            ResourceFromExt::None => {
                ResourceToExt::None
            }
            ResourceFromExt::Ext(raw) => {
                ResourceToExt::Ext(raw.clone())
            }
        }
    }
}

impl ResourceToExt
{
    pub fn reverse(&self)->ResourceFromExt {
        match self {
            ResourceToExt::None => {
                ResourceFromExt::None
            }
            ResourceToExt::Ext(raw) => {
                ResourceFromExt::Ext(raw.clone())
            }
        }
    }
}


#[derive(Clone,Serialize,Deserialize)]
pub enum ActorFromExt
{
    None,
    Ext(Raw)
}






pub struct ActorGathering
{
    pub key: ResourceKey,
    pub entity: Vec<ResourceKey>
}


pub struct ActorWatcher
{
    pub entity: ResourceKey,
    pub tx: Sender<Event>
}

impl ActorWatcher
{
    pub fn new(entity: ResourceKey) ->(Self, broadcast::Receiver<Event>)
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
    pub key: ResourceKey,
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
    pub fn new(app:AppKey, seq: u64, index: u64, tx: mpsc::Sender<ActorKey>) ->Self {
        ActorKeySeq{
            app: app,
            seq: seq,
            index: index,
            tx: tx
        }
    }

    pub async fn next(&mut self)-> ActorKey
    {
        self.index=self.index+1;
        let key = ActorKey::new(self.app.clone(), Id::new(self.seq, self.index ));

        self.tx.send(key.clone() ).await;

        key
    }
}

pub struct ResourceMessageWrapper
{
    pub user: UserKey,
    pub sub_space: SubSpaceKey,
    pub message: ResourceMessage
}


#[derive(Clone,Serialize,Deserialize)]
pub struct ResourceTo
{
    pub key: ResourceKey
}

impl ResourceTo{
    pub fn new(key:ResourceKey) -> Self{
        ResourceTo{
            key: key
        }
    }
}


pub struct ResourceMessageBuilder{
    pub from: Option<ResourceFrom>,
    pub to: Option<ResourceTo>,
    pub payload: ResourceMessagePayload,
    pub reply: Option<oneshot::Sender<Result<ResourceMessage,Fail>>>
}

impl ResourceMessageBuilder{
    pub fn new()->Self{

        ResourceMessageBuilder{
            from: Option::None,
            to: Option::None,
            payload: ResourceMessagePayload::None,
            reply: Option::None
        }
    }

    pub fn build(self)->Result<ResourceMessage,Error>{
        if let &ResourceMessagePayload::None = &self.payload {
            return Err("ResourceMessagePayload cannot be None".into());
        }

        Ok(ResourceMessage{
            from: self.from.ok_or("need to set 'from' in ResourceMessageBuilder")?,
            to: self.to.ok_or("need to set 'to' in ResourceMessageBuilder")?,
            payload: self.payload,
        })
    }

    pub fn injected(&mut self) {
        self.from = Option::Some(ResourceFrom::Injected);
    }

    pub fn to(&mut self, to: ResourceTo) {
        self.to = Option::Some(to);
    }

    pub fn from(&mut self, from: ResourceFrom ) {
        self.from = Option::Some(from);
    }

    pub fn reply(&mut self) -> oneshot::Receiver<Result<ResourceMessage,Fail>> {
        let (tx,rx) = oneshot::channel();
        self.reply = Option::Some(tx);
        rx
    }
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ResourceMessage
{
    pub from: ResourceFrom,
    pub to: ResourceTo,
    pub payload: ResourceMessagePayload
}

impl ResourceMessage
{
    pub fn verify_type(&self, resource_type: ResourceType )->Result<(),Fail>
    {
        if self.to.key.resource_type() == resource_type {
            Ok(())
        } else {
            Err(Fail::WrongResourceType{
                received: resource_type,
                expected: HashSet::from_iter(vec![self.to.key.resource_type().clone()])
            })
        }
    }
}

#[derive(Clone,Serialize,Deserialize)]
pub enum ResourceMessagePayload {
    None,
    Raw(Arc<RawPayload>),
    Request(ResourceRequest),
    Response(ResourceResponse)
}

#[derive(Clone,Serialize,Deserialize)]
pub enum ResourceRequest
{
    Create(ResourceCreate),
    Select(ResourceSelector),
    Unique(ResourceType)
}

#[derive(Clone,Serialize,Deserialize)]
pub enum ResourceResponse
{
    Resource(Option<ResourceRecord>),
    Resources(Vec<ResourceRecord>),
    Unique(ResourceId)
}

pub type Raw=Vec<u8>;
pub type RawPayload=Vec<u8>;
pub type RawState=Vec<u8>;

