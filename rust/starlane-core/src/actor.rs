use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::iter::FromIterator;
use std::marker::PhantomData;
use std::str::FromStr;
use std::sync::Arc;

use serde::{Deserialize, Serialize, Serializer};
use serde::de::DeserializeOwned;
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::sync::broadcast::Sender;

use crate::app::ConfigSrc;
use crate::error::Error;
use crate::frame::Event;
use crate::id::Id;
use crate::keys::{AppKey, ResourceId, ResourceKey, SubSpaceKey, UserKey};
use crate::message::Fail;
use crate::names::Name;
use crate::resource::{
    Labels, Names, ResourceAddress, ResourceArchetype, ResourceAssign,
    ResourceCreate, ResourceKind, ResourceRecord, ResourceRegistration, ResourceRegistryInfo,
    ResourceSelector, ResourceStub, ResourceType,
};
use crate::resource::address::{ResourceAddressPart, SkewerCase};
use crate::resource::ResourceAddressPartKind::Base64Encoded;
use crate::star::StarKey;

#[derive(Debug, Eq, PartialEq, Hash, Clone, Serialize, Deserialize)]
pub struct ActorKey {
    pub app: AppKey,
    pub id: Id,
}

impl ActorKey {
    pub fn address_part(&self) -> Result<ResourceAddressPart, Error> {
        Ok(ResourceAddressPart::SkewerCase(SkewerCase::new(
            self.id.to_string().as_str(),
        )?))
    }
}

impl ActorKey {
    pub fn new(app: AppKey, id: Id) -> Self {
        ActorKey { app: app, id: id }
    }
}

impl ToString for ActorKey {
    fn to_string(&self) -> String {
        format!("{}-{}", self.app.to_string(), self.id.to_string())
    }
}

impl FromStr for ActorKey {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let pos = s.rfind('-').ok_or("expected '-' between parent and id")?;
        let (parent, id) = s.split_at(pos);
        let app = AppKey::from_str(parent)?;
        let id = Id::from_str(id)?;
        Ok(ActorKey { app: app, id: id })
    }
}

pub type ActorSpecific = Name;
pub type GatheringSpecific = Name;

#[derive(Debug,Eq, PartialEq, Hash, Clone, Serialize, Deserialize)]
pub enum ActorKind {
    Stateful,
    Stateless,
}

impl ActorKind {
    // it looks a little pointless but helps get around a compiler problem with static_lazy values
    pub fn as_kind(&self) -> Self {
        self.clone()
    }
}

impl fmt::Display for ActorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ActorKind::Stateful => "Stateful".to_string(),
                ActorKind::Stateless => "Stateless".to_string(),
            }
        )
    }
}

impl FromStr for ActorKind {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Stateful" => Ok(ActorKind::Stateful),
            "Stateless" => Ok(ActorKind::Stateless),
            _ => Err(format!("could not find ActorKind: {}", s).into()),
        }
    }
}
