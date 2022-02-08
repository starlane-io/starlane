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
use mesh_portal_serde::version::latest::command::common::StateSrc;
use mesh_portal_serde::version::latest::entity::request::create::KindTemplate;

use mesh_portal_serde::version::latest::id::{Address, KindParts, ResourceKind, Specific};
use mesh_portal_serde::version::latest::payload::Payload;
use mesh_portal_serde::version::latest::resource::{ResourceStub, Status};
use mesh_portal_versions::version::v0_0_1::pattern::parse::consume_kind;
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

use crate::message::{MessageExpect, ProtoStarMessage, ReplyKind};
use crate::names::Name;
use crate::star::{StarInfo, StarKey, StarSkel};
use crate::star::shell::wrangler::{StarWrangle};
use crate::starlane::api::StarlaneApi;
use crate::util::AsyncHashMap;

pub mod artifact;
pub mod config;
pub mod file;


#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResourceLocation {
    Unassigned,
    Host(StarKey)
}

impl ToString for ResourceLocation {
    fn to_string(&self) -> String {
        match self {
            ResourceLocation::Unassigned => {
                "Unassigned".to_string()
            }
            ResourceLocation::Host(host) => {
                host.to_string()
            }
        }
    }
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
              kind: Kind::Root.to_resource_kind(),
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
    Debug,
    Clone,
    Serialize,
    Deserialize,
    Eq,
    PartialEq,
    Hash,
    strum_macros::Display,
    strum_macros::EnumString
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
    Control,
    Proxy,
    Credentials,
}

/*
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
               "Control" => Self::Control,
               what => {
                   return Err(format!("invalid ResourceType: '{}'", what).into());
               }
           }
        )
    }
}
 */

impl Into<String> for ResourceType {
    fn into(self) -> String {
        self.to_string()
    }
}

impl TryFrom<String> for ResourceType {
    type Error = mesh_portal_serde::error::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Ok(ResourceType::from_str(value.as_str())?)
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
    Control
}

impl Kind {

    pub fn to_resource_kind(self) -> KindParts {
        self.into()
    }

}

impl TryInto<KindTemplate> for Kind {
    type Error = mesh_portal_serde::error::Error;

    fn try_into(self) -> Result<KindTemplate, Self::Error> {
        Ok(KindTemplate {
            resource_type: self.resource_type().to_string(),
            kind: self.sub_string(),
            specific: match self.specific() {
                None => None,
                Some(specific) => Some(specific.try_into()?)
            }
        })
    }
}

impl TryFrom<String> for Kind {
    type Error =mesh_portal_serde::error::Error;

    fn try_from(kind: String) -> Result<Self, Self::Error> {
        match Kind::from_str(kind.as_str()) {
            Ok(kind) => {
                Ok(kind)
            }
            Err(error) => {
                Err(mesh_portal_serde::error::Error{
                    message: error.to_string()
                })
            }
        }
    }
}

impl TryInto<String> for Kind {
    type Error =mesh_portal_serde::error::Error;

    fn try_into(self) -> Result<String, Self::Error> {
        Ok(self.to_string())
    }
}

impl ToString for Kind {
    fn to_string(&self) -> String {
        let parts: KindParts = self.clone().into();
        parts.to_string()
    }
}


/*
impl TryInto<mesh_portal_serde::version::latest::id::ResourceKind> for Kind {
    type Error =  mesh_portal_serde::error::Error;

    fn try_into(self) -> Result<mesh_portal_serde::version::latest::id::ResourceKind, Self::Error> {
        let parts: KindParts = self.into();

        Ok(mesh_portal_serde::version::latest::id::ResourceKind {
            resource_type: parts.resource_type.into(),
            kind: parts.kind,
            specific: parts.specific
        })
    }
}
 */


impl TryFrom<KindParts> for Kind {
    type Error = mesh_portal_serde::error::Error;

    fn try_from(parts: KindParts) -> Result<Self, Self::Error> {
        match ResourceType::from_str(parts.resource_type.as_str() )? {
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

        Ok(match ResourceType::from_str(parts.resource_type.as_str())? {
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
        let resource_kind = consume_kind(s)?;
        Ok(resource_kind.try_into()?)
    }
}

impl Into<KindParts> for Kind {
    fn into(self) -> KindParts {
        KindParts {
            resource_type: self.resource_type().to_string(),
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
            Kind::Control => ResourceType::Control
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
            ResourceType::Control => Self::Control
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
    Repo,
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
    ResourceConfig,
    Bind,
    Wasm,
    Dir,
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


