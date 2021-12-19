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
use crate::mesh::serde::id::{Address, KindParts};
use crate::mesh::serde::pattern::AddressKindPattern;
use crate::mesh::serde::payload::{PayloadMap, Primitive, RcCommand};
use crate::mesh::serde::payload::Payload;
use crate::mesh::serde::resource::{Archetype, ResourceStub, Status};
use crate::mesh::serde::resource::command::common::{SetProperties, SetRegistry, StateSrc};
use crate::mesh::serde::resource::command::create::{Create, Strategy};
use crate::mesh::serde::resource::command::create::AddressSegmentTemplate;
use crate::mesh::serde::resource::command::select::Select;
use crate::mesh::serde::resource::command::update::Update;
use crate::message::{MessageExpect, ProtoStarMessage, ReplyKind};
use crate::names::Name;
use crate::resources::message::{MessageFrom, ProtoRequest};
use crate::star::{StarInfo, StarKey, StarSkel};
use crate::star::shell::wrangler::{StarWrangle};
use crate::starlane::api::StarlaneApi;
use crate::util::AsyncHashMap;
use mesh_portal_serde::version::v0_0_1::pattern::parse::consume_kind;

pub mod artifact;
pub mod config;
pub mod file;


#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResourceLocation {
    Unassigned,
    Host(StarKey)
}

impl ResourceLocation {
    pub fn new(star: StarKey) -> Self {
        ResourceLocation::Host( star )
    }
    pub fn root() -> Self {
        ResourceLocation::Host(StarKey::central())
    }

    pub fn ok_or(&self)->Result<StarKey,Error> {
        match self {
            ResourceLocation::Unassigned => {
                Err("ResourceLocation is unassigned".into())
            }
            ResourceLocation::Host(star) => {
                Ok(star.clone())
            }
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
            stub: ResourceStub {
              address: Address::root(),
              kind: Kind::Root,
              properties: Default::default(),
              status: Status::Ready
            },
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

impl FromStr for ResourceType{
    type Err = mesh_portal_serde::error::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(
           match s  {
               "Root" => Self::Root,
               "Space" => Self::Space,
               "Base" => Self::Base,
               "User" => Self::User,
               "App" => Self::App,
               "Mechtron" => Self::Mechtron,
               "FileSystem" => Self::FileSystem,
               "File" => Self::File,
               "Database" => Self::Database,
               "Authenticator" => Self::Authenticator,
               "ArtifactBundleSeries" => Self::ArtifactBundleSeries,
               "ArtifactBundle" => Self::ArtifactBundle,
               "Artifact" => Self::Artifact,
               "Proxy" => Self::Proxy,
               "Credentials" => Self::Credentials,
               what => {
                   return Err(format!("invalid ResourceType: '{}'", what).into());
               }
           }
        )
    }
}

impl Into<String> for ResourceType {
    fn into(self) -> String {
        self.to_string()
    }
}

impl TryFrom<String> for ResourceType {
    type Error = mesh_portal_serde::error::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        ResourceType::from_str(value.as_str() )
    }
}


#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    Eq,
    PartialEq,
    Hash,
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

impl ToString for Kind {
    fn to_string(&self) -> String {
        let parts: KindParts = self.clone().into();
        parts.to_string()
    }
}

impl TryFrom<KindParts> for Kind {
    type Error = mesh_portal_serde::error::Error;

    fn try_from(parts: KindParts) -> Result<Self, Self::Error> {
        match parts.resource_type {
            ResourceType::Base => {
                let parts: String = match parts.kind {
                    None => {
                        return Err("expected parts".into());
                    }
                    Some(parts) => {
                        return Ok(Self::Base(BaseKind::from_str(parts.as_str())?));
                    }
                };
            }
            ResourceType::Database => {
                match parts.kind
                {
                    None => {
                        return Err("expected kind".into());
                    }
                    Some(kind) => {
                        match kind.as_str() {
                            "Relational" => {
                                match parts.specific {
                                    None => {
                                        return Err("expected specific".into());
                                    }
                                    Some(specific) => {
                                        return Ok(Kind::Database(DatabaseKind::Relational(specific)));
                                    }
                                }
                            }
                            what => {
                                return Err(format!("Database type does not have a Kind {}", what).into());
                            }
                    }
                }

                }
            }
            ResourceType::Artifact => {
                match parts.kind {
                    None => {
                        return Err("kind needs to be set".into())
                    }
                    Some(kind)  => {
                        return Ok(Self::Artifact(ArtifactKind::from_str(kind.as_str())?))
                    }
                }
            }
            _ => {}
        }

        Ok(match parts.resource_type {
            ResourceType::Root => {Self::Root}
            ResourceType::Space => {Self::Space}
            ResourceType::User => {Self::User}
            ResourceType::App => {Self::App}
            ResourceType::Mechtron => {Self::Mechtron}
            ResourceType::FileSystem => {Self::FileSystem}
            ResourceType::Authenticator => {Self::Authenticator}
            ResourceType::ArtifactBundleSeries => {Self::ArtifactBundleSeries}
            ResourceType::ArtifactBundle => {Self::ArtifactBundle}
            ResourceType::Proxy => {Self::Proxy}
            ResourceType::Credentials => {Self::Credentials}
            what => { return Err(format!("missing Kind from_str for: {}",what.to_string()).into())}
        })
    }
}

impl FromStr for Kind {
    type Err = mesh_portal_serde::error::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok( consume_kind(s)? )
    }
}

impl Into<KindParts> for Kind {
    fn into(self) -> KindParts {
        KindParts {
            resource_type: self.resource_type(),
            kind: self.sub_string(),
            specific: self.specific()
        }
    }
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
                match kind {
                    None => {
                        return Err("expected kind".into());
                    }
                    Some(kind) => {
                        return Ok(Self::Base(BaseKind::from_str(kind.as_str())?));
                    }
                }
            }
            ResourceType::User => { Self::User}
            ResourceType::App => {Self::App}
            ResourceType::Mechtron => {Self::Mechtron}
            ResourceType::FileSystem => {Self::FileSystem}
            ResourceType::File => {
                let kind = match kind.ok_or("expected sub kind".into() ){
                    Ok(kind) => {
                        return Ok(Self::File(FileKind::from_str(kind.as_str())?));
                    }
                    Err(err) => {
                        return Err(err);
                    }
                };

            }
            ResourceType::Database => {
                match kind.ok_or("expected sub kind".into() )
                {
                    Ok(kind) => {
                        if "Relational" != kind.as_str() {
                            return Err(format!("DatabaseKind is not recognized found: {}",kind).into());
                        }
                        match specific.ok_or("expected specific".into() ) {
                            Ok(specific) => {
                                return Ok(Self::Database(DatabaseKind::Relational(specific)));
                            }
                            Err(err) => {
                                return Err(err)
                            }
                        }
                    }
                    Err(err) => {
                        return Err(err);
                    }
                }

            }
            ResourceType::Authenticator => {Self::Authenticator}
            ResourceType::ArtifactBundleSeries => {Self::ArtifactBundleSeries}
            ResourceType::ArtifactBundle => {Self::ArtifactBundle}
            ResourceType::Artifact => {
                match kind {
                    None => {
                        return Err("kind needs to be set".into());
                    }
                    Some(kind) => {
                        return Ok(Self::Artifact(ArtifactKind::from_str(kind.as_str())?));
                    }
                };
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    pub stub: ResourceStub,
    pub state: Payload,
}

impl Resource {
    pub fn new(stub: ResourceStub, state: Payload) -> Resource {
        Resource {
            stub,
            state
        }
    }

    pub fn address(&self) -> Address {
        self.stub.address.clone()
    }

    pub fn resource_type(&self) -> ResourceType {
        self.stub.kind.resource_type()
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


#[derive(Debug, Clone, Serialize, Deserialize, strum_macros::Display)]
pub enum AssignKind {
    Create,
    // eventually we will have Move as well as Create
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceAssign {
    pub kind: AssignKind,
    pub stub: ResourceStub,
    pub state: StateSrc,
}


impl ResourceAssign {

    pub fn new(kind: AssignKind, stub: ResourceStub, state: StateSrc) -> Self {
        Self {
            kind,
            stub,
            state
        }
    }

}


