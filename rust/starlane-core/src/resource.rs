use std::collections::{HashMap, HashSet};
use std::collections::hash_map::RandomState;
use std::convert::{TryFrom, TryInto};
use std::fmt;
use std::fmt::{Debug, Formatter};
use std::fs::DirBuilder;
use std::hash::Hash;
use std::iter::FromIterator;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use mesh_portal_serde::version::latest::id::Specific;
use mesh_portal_serde::version::v0_0_1::generic::entity::request::ReqEntity;
use mesh_portal_serde::version::v0_0_1::pattern::SegmentPattern;
use rusqlite::{Connection, params, params_from_iter, Row, ToSql, Transaction};
use rusqlite::types::{ToSqlOutput, Value, ValueRef};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot};
use tokio::sync::oneshot::Receiver;
use tracing_futures::WithSubscriber;

use crate::{error, logger, util};
use crate::error::Error;
use crate::fail::Fail;
use crate::file_access::FileAccess;
use crate::frame::{ResourceHostAction, StarMessagePayload};
use crate::logger::{elog, LogInfo, StaticLogInfo};
use crate::mesh::serde::entity::request::Rc;
use crate::mesh::serde::fail;
use crate::mesh::serde::id::Address;
use crate::mesh::serde::pattern::AddressKindPattern;
use crate::mesh::serde::payload::{PayloadMap, Primitive, RcCommand};
use crate::mesh::serde::payload::Payload;
use crate::mesh::serde::resource::{Archetype, ResourceStub};
use crate::mesh::serde::resource::command::common::{SetProperties, SetRegistry};
use crate::mesh::serde::resource::command::create::{Create, Strategy};
use crate::mesh::serde::resource::command::create::AddressSegmentTemplate;
use crate::mesh::serde::resource::command::select::Select;
use crate::mesh::serde::resource::command::update::Update;
use crate::message::{MessageExpect, ProtoStarMessage, ReplyKind};
use crate::names::Name;
use crate::resources::message::{MessageFrom, ProtoMessage};
use crate::star::{StarInfo, StarKey, StarSkel};
use crate::star::shell::pledge::{ResourceHostSelector, StarConscript};
use crate::starlane::api::StarlaneApi;
use crate::util::AsyncHashMap;

pub mod artifact;
pub mod config;
pub mod file;
pub mod file_system;
pub mod user;


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLocation {
    pub host: StarKey,
}

impl ResourceLocation {
    pub fn new(star: StarKey) -> Self {
        Self { host: star }
    }
    pub fn root() -> Self {
        Self {
            host: StarKey::central(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct DisplayValue {
    string: String,
}

impl DisplayValue {
    pub fn new(string: &str) -> Result<Self, Error> {
        if string.is_empty() {
            return Err("cannot be empty".into());
        }

        Ok(DisplayValue {
            string: string.to_string(),
        })
    }
}

impl ToString for DisplayValue {
    fn to_string(&self) -> String {
        self.string.clone()
    }
}

impl FromStr for DisplayValue {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(DisplayValue::new(s)?)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceRecord {
    pub stub: ResourceStub,
    pub location: ResourceLocation,
}

impl ResourceRecord {
    pub fn new(stub: ResourceStub, host: StarKey) -> Self {
        ResourceRecord {
            stub: stub,
            location: ResourceLocation::new(host),
        }
    }

    pub fn root() -> Self {
        Self {
            stub: ResourceStub::root(),
            location: ResourceLocation::root(),
        }
    }
}

impl Into<ResourceStub> for ResourceRecord {
    fn into(self) -> ResourceStub {
        self.stub
    }
}

#[derive(
    Clone,
    Serialize,
    Deserialize,
    Eq,
    PartialEq,
    Hash,
    strum_macros::Display,
    strum_macros::EnumString,
)]
pub enum ResourceType {
    Root,
    Space,
    Base,
    User,
    App,
    Mechtron,
    FileSystem,
    File,
    Database,
    Authenticator,
    ArtifactBundleSeries,
    ArtifactBundle,
    Artifact,
    Proxy,
    Credentials,
}

#[derive(
    Clone,
    Serialize,
    Deserialize,
    Eq,
    PartialEq,
    Hash,
    strum_macros::Display,
    strum_macros::EnumString,
)]
pub enum Kind {
    Root,
    Space,
    Base(BaseKind),
    User,
    App,
    Mechtron,
    FileSystem,
    File(FileKind),
    Database(DatabaseKind),
    Authenticator,
    ArtifactBundleSeries,
    ArtifactBundle,
    Artifact(ArtifactKind),
    Proxy,
    Credentials,
}

impl Kind {
    pub fn resource_type(&self) -> ResourceType {
        match self {
            Kind::Root => ResourceType::Root,
            Kind::Space => ResourceType::Space,
            Kind::Base(_) => ResourceType::Base,
            Kind::User => ResourceType::User,
            Kind::App => ResourceType::App,
            Kind::Mechtron => ResourceType::Mechtron,
            Kind::FileSystem => ResourceType::FileSystem,
            Kind::File(_) => ResourceType::File,
            Kind::Database(_) => ResourceType::Database,
            Kind::Authenticator => ResourceType::Authenticator,
            Kind::ArtifactBundleSeries => ResourceType::ArtifactBundleSeries,
            Kind::ArtifactBundle => ResourceType::ArtifactBundle,
            Kind::Artifact(_) => ResourceType::Artifact,
            Kind::Proxy => ResourceType::Proxy,
            Kind::Credentials => ResourceType::Credentials,
        }
    }

    pub fn sub_string(&self) -> Option<String> {
        match self {
            Self::Base(base) =>  {
                Option::Some(base.to_string())
            }
            Self::File( file ) => {
                Option::Some(file.to_string())
            }
            Self::Database( db) => {
                Option::Some(db.to_string())
            }
            Self::Artifact( artifact) => {
                Option::Some(artifact.to_string())
            }
            _ => {
                Option::None
            }
        }
    }

    pub fn specific(&self) -> Option<Specific> {
        match self {
            Self::Database(kind) => kind.specific(),
            _ => Option::None,
        }
    }

    pub fn from( resource_type: ResourceType, kind: Option<String>, specific: Option<Specific> ) -> Result<Self,Error> {
        Ok(match resource_type {
            ResourceType::Root => {Self::Root}
            ResourceType::Space => {Self::Space}
            ResourceType::Base => {
                let kind = kind.ok_or("expected sub kind".into() )?;
                Self::Base(BaseKind::from_str(kind)?)
            }
            ResourceType::User => { Self::User}
            ResourceType::App => {Self::App}
            ResourceType::Mechtron => {Self::Mechtron}
            ResourceType::FileSystem => {Self::FileSystem}
            ResourceType::File => {Self::File}
            ResourceType::Database => {
                let kind = kind.ok_or("expected sub kind".into() )?;
                if "Relational" != kind.as_str() {
                    return Err(format!("DatabaseKind is not recognized found: {}",kind).into());
                }
                let specific = specific.ok_or("expected specific".into() )?;
                Self::Database(DatabaseKind::Relational(specific))
            }
            ResourceType::Authenticator => {Self::Authenticator}
            ResourceType::ArtifactBundleSeries => {Self::ArtifactBundleSeries}
            ResourceType::ArtifactBundle => {Self::ArtifactBundle}
            ResourceType::Artifact => {
                let kind = kind.ok_or("expected sub kind".into() )?;
                Self::Artifact(ArtifactKind::from_str(kind)?)
            }
            ResourceType::Proxy => {Self::Proxy}
            ResourceType::Credentials => {Self::Credentials}
        })
    }
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Hash,
    Serialize,
    Deserialize,
    strum_macros::Display,
)]
pub enum DatabaseKind {
    Relational(Specific),
}

impl DatabaseKind {
    pub fn specific(&self) -> Option<Specific> {
        match self {
            Self::Relational(specific) => Option::Some(specific.clone()),
            _ => Option::None,
        }
    }
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Hash,
    Serialize,
    Deserialize,
    strum_macros::Display,
    strum_macros::EnumString,
)]
pub enum BaseKind {
    User,
    App,
    Mechtron,
    Database,
    Any,
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Hash,
    Serialize,
    Deserialize,
    strum_macros::Display,
    strum_macros::EnumString,
)]
pub enum FileKind {
    File,
    Dir,
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Hash,
    Serialize,
    Deserialize,
    strum_macros::Display,
    strum_macros::EnumString,
)]
pub enum ArtifactKind {
    Raw,
    AppConfig,
    MechtronConfig,
    BindConfig,
    Wasm,
    HttpRouter,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    pub address: Address,
    pub archetype: ResourceArchetype,
    pub state: Payload,
}

impl Resource {
    pub fn new(address: Address, archetype: ResourceArchetype, state: Payload) -> Resource {
        Resource {
            address: address,
            state: state_src,
            archetype: archetype,
        }
    }

    pub fn address(&self) -> Address {
        self.address.clone()
    }

    pub fn resource_type(&self) -> ResourceType {
        self.key.resource_type()
    }

    pub fn state_src(&self) -> Payload {
        self.state.clone()
    }
}

/// can have other options like to Initialize the state data
#[derive(Debug, Clone, Serialize, Deserialize, strum_macros::Display)]
pub enum AssignResourceStateSrc {
    Stateless,
    Direct(Payload),
}


pub enum AssignKind {
    Create,
    // eventually we will have Move as well as Create
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceAssign<S> {
    pub kind: AssignKind,
    pub stub: ResourceStub,
    pub state_src: S,
}


impl<S> ResourceAssign<S> {

    pub fn new( kind: AssignKind, stub: ResourceStub, state_src: S ) -> Self {
        Self {
            kind,
            stub,
            state_src
        }
    }

    pub fn archetype(&self) -> Archetype {
        self.stub.archetype.clone()
    }
}
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct ResourceKindParts {
    pub resource_type: String,
    pub kind: Option<String>,
    pub specific: Option<Specific>,
}

impl ToString for ResourceKindParts {
    fn to_string(&self) -> String {
        if self.specific.is_some() && self.kind.is_some() {
            format!(
                "<{}<{}<{}>>>",
                self.resource_type,
                self.kind.as_ref().unwrap().to_string(),
                self.specific.as_ref().unwrap().to_string()
            )
        } else if self.kind.is_some() {
            format!(
                "<{}<{}>>",
                self.resource_type,
                self.kind.as_ref().unwrap().to_string()
            )
        } else {
            format!("<{}>", self.resource_type)
        }
    }
}

impl FromStr for ResourceKindParts {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (leftover, rtn) = parse_kind(s)?;
        if leftover.len() > 0 {
            return Err(format!(
                "ResourceKindParts ERROR: could not parse extra: '{}' in string '{}'",
                leftover, s
            )
            .into());
        }
        Ok(rtn)
    }
}
