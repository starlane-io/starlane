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
use mesh_portal::version::latest::command::common::StateSrc;
use mesh_portal::version::latest::entity::request::create::KindTemplate;

use mesh_portal::version::latest::id::{KindParts, Point, ResourceKind, Specific};
use mesh_portal::version::latest::payload::Payload;
use mesh_portal::version::latest::particle::{Status, Stub};
use mesh_portal::version::latest::security::Permissions;
use mesh_portal_versions::version::v0_0_1::parse::consume_kind;
use mesh_portal_versions::version::v0_0_1::particle::particle::Details;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot};
use tokio::sync::oneshot::Receiver;
use tracing_futures::WithSubscriber;
use cosmic_nom::new_span;
use mesh_portal::error::MsgErr;
use mesh_portal::version::latest::config::{ParticleConfigBody, PointConfig};
use mesh_portal_versions::version::v0_0_1::id::id::ToPoint;
use mesh_portal_versions::version::v0_0_1::sys::{AssignmentKind, ChildRegistry, Location};

use crate::{error, logger, util};
use crate::config::config::ParticleConfig;
use crate::error::Error;
use crate::fail::Fail;
use crate::file_access::FileAccess;
use crate::frame::{ResourceHostAction, StarMessagePayload};
use crate::logger::{elog, LogInfo, StaticLogInfo};

use crate::message::{MessageExpect, ProtoStarMessage, ReplyKind};
use crate::names::Name;
use crate::particle::KindBase::Mechtron;
use crate::particle::property::{AnythingPattern, BoolPattern, EmailPattern, PointPattern, PropertiesConfig, PropertyPermit, PropertySource, U64Pattern};
use crate::star::{StarInfo, StarKey, StarSkel};
use crate::star::core::particle::driver::user::UsernamePattern;
use crate::util::AsyncHashMap;

pub mod artifact;
pub mod config;
pub mod file;
pub mod property;

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct DisplayValue {
    string: String,
}

pub fn root_location() -> Location {
    Location::new(StarKey::central().to_point() )
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

#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    Eq,
    PartialEq,
    Hash,
    strum_macros::Display,
    strum_macros::EnumString,
)]
pub enum KindBase {
    Root,
    Space,
    UserBase,
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
impl FromStr for KindBase{
    type Err = mesh_portal::error::MsgErr;

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
                   return Err(format!("invalid Kind: '{}'", what).into());
               }
           }
        )
    }
}

 */

impl Into<String> for KindBase {
    fn into(self) -> String {
        self.to_string()
    }
}

impl TryFrom<String> for KindBase {
    type Error = mesh_portal::error::MsgErr;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Ok(KindBase::from_str(value.as_str())?)
    }
}

impl KindBase {
    pub fn child_resource_registry_handler(&self) -> ChildRegistry {
        match self {
            Self::UserBase => ChildRegistry::Core,
            _ => ChildRegistry::Shell
        }
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
    UserBase(UserBaseSubKind),
    Base(BaseSubKind),
    User,
    App,
    Mechtron,
    FileSystem,
    File(FileSubKind),
    Database(DatabaseSubKind),
    Authenticator,
    ArtifactBundleSeries,
    ArtifactBundle,
    Artifact(ArtifactSubKind),
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
    type Error = mesh_portal::error::MsgErr;

    fn try_into(self) -> Result<KindTemplate, Self::Error> {
        Ok(KindTemplate {
            kind: self.kind().to_string(),
            sub_kind: self.sub_kind(),
            specific: match self.specific() {
                None => None,
                Some(specific) => Some(specific.try_into()?)
            }
        })
    }
}

impl TryFrom<String> for Kind {
    type Error =mesh_portal::error::MsgErr;

    fn try_from(kind: String) -> Result<Self, Self::Error> {
        Kind::from_str(kind.as_str())
    }
}

impl TryInto<String> for Kind {
    type Error =mesh_portal::error::MsgErr;

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
impl TryInto<mesh_portal::version::latest::id::ResourceKind> for Kind {
    type Error =  mesh_portal::error::Error;

    fn try_into(self) -> Result<mesh_portal::version::latest::id::ResourceKind, Self::Error> {
        let parts: KindParts = self.into();

        Ok(mesh_portal::version::latest::id::ResourceKind {
            kind: parts.kind.into(),
            kind: parts.kind,
            specific: parts.specific
        })
    }
}
 */


impl TryFrom<KindParts> for Kind {
    type Error = mesh_portal::error::MsgErr;

    fn try_from(parts: KindParts) -> Result<Self, Self::Error> {
        match KindBase::from_str(parts.kind.as_str() )? {
            KindBase::Base => {
                match parts.sub_kind {
                    None => {
                        return Err("expected parts".into());
                    }
                    Some(parts) => {
                        return Ok(Self::Base(BaseSubKind::from_str(parts.as_str())?));
                    }
                }
            }
            KindBase::Database => {
                match parts.sub_kind
                {
                    None => {
                        return Err("expected kind".into());
                    }
                    Some(sub_kind) => {
                        match sub_kind.as_str() {
                            "Relational" => {
                                match parts.specific {
                                    None => {
                                        return Err("expected specific".into());
                                    }
                                    Some(specific) => {
                                        return Ok(Kind::Database(DatabaseSubKind::Relational(specific)));
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
            KindBase::Artifact => {
                match parts.sub_kind {
                    None => {
                        return Err("kind needs to be set".into())
                    }
                    Some(sub_kind)  => {
                        return Ok(Self::Artifact(ArtifactSubKind::from_str(sub_kind.as_str())?))
                    }
                }
            }
            KindBase::UserBase=> {
                match parts.sub_kind {
                    None => {
                        return Err("kind needs to be set for UserBase".into())
                    }
                    Some(sub_kind)  => {
                        return Ok(Self::UserBase(UserBaseSubKind::from_str(sub_kind.as_str())?))
                    }
                }
            }
            _ => {}
        }

        Ok(match KindBase::from_str(parts.kind.as_str())? {
            KindBase::Root => {Self::Root}
            KindBase::Space => {Self::Space}
            KindBase::User => {Self::User}
            KindBase::App => {Self::App}
            KindBase::Mechtron => {Self::Mechtron}
            KindBase::FileSystem => {Self::FileSystem}
            KindBase::Authenticator => {Self::Authenticator}
            KindBase::ArtifactBundleSeries => {Self::ArtifactBundleSeries}
            KindBase::ArtifactBundle => {Self::ArtifactBundle}
            KindBase::Proxy => {Self::Proxy}
            KindBase::Credentials => {Self::Credentials}
            what => { return Err(format!("missing Kind from_str for: {}",what.to_string()).into())}
        })
    }
}

impl FromStr for Kind {
    type Err = mesh_portal::error::MsgErr;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let resource_kind = consume_kind(new_span(s))?;
        Ok(resource_kind.try_into()?)
    }
}

impl Into<KindParts> for Kind {
    fn into(self) -> KindParts {
        KindParts {
            kind: self.kind().to_string(),
            sub_kind: self.sub_kind(),
            specific: self.specific()
        }
    }
}

impl Kind {
    pub fn kind(&self) -> KindBase {
        match self {
            Kind::Root => KindBase::Root,
            Kind::Space => KindBase::Space,
            Kind::Base(_) => KindBase::Base,
            Kind::User => KindBase::User,
            Kind::App => KindBase::App,
            Kind::Mechtron => KindBase::Mechtron,
            Kind::FileSystem => KindBase::FileSystem,
            Kind::File(_) => KindBase::File,
            Kind::Database(_) => KindBase::Database,
            Kind::Authenticator => KindBase::Authenticator,
            Kind::ArtifactBundleSeries => KindBase::ArtifactBundleSeries,
            Kind::ArtifactBundle => KindBase::ArtifactBundle,
            Kind::Artifact(_) => KindBase::Artifact,
            Kind::Proxy => KindBase::Proxy,
            Kind::Credentials => KindBase::Credentials,
            Kind::Control => KindBase::Control,
            Kind::UserBase(_) => KindBase::UserBase
        }
    }

    pub fn sub_kind(&self) -> Option<String> {
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
            Self::UserBase( kind) => {
                Option::Some(kind.to_string())
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

    pub fn from(kind: KindBase, sub_kind: Option<String>, specific: Option<Specific> ) -> Result<Self,Error> {
        Ok(match kind {
            KindBase::Root => {Self::Root}
            KindBase::Space => {Self::Space}
            KindBase::Base => {
                match sub_kind {
                    None => {
                        return Err("expected kind".into());
                    }
                    Some(kind) => {
                        return Ok(Self::Base(BaseSubKind::from_str(kind.as_str())?));
                    }
                }
            }
            KindBase::User => { Self::User}
            KindBase::App => {Self::App}
            KindBase::Mechtron => {Self::Mechtron}
            KindBase::FileSystem => {Self::FileSystem}
            KindBase::File => {
                let sub_kind = match sub_kind.ok_or("expected sub kind".into() ){
                    Ok(sub_kind) => {
                        return Ok(Self::File(FileSubKind::from_str(sub_kind.as_str())?));
                    }
                    Err(err) => {
                        return Err(err);
                    }
                };

            }
            KindBase::Database => {
                match sub_kind.ok_or("expected sub kind".into() )
                {
                    Ok(sub_kind) => {
                        if "Relational" != sub_kind.as_str() {
                            return Err(format!("DatabaseKind is not recognized found: {}",sub_kind).into());
                        }
                        match specific.ok_or("expected Database<Relational<specific>>".into() ) {
                            Ok(specific) => {
                                return Ok(Self::Database(DatabaseSubKind::Relational(specific)));
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
            KindBase::Authenticator => {Self::Authenticator}
            KindBase::ArtifactBundleSeries => {Self::ArtifactBundleSeries}
            KindBase::ArtifactBundle => {Self::ArtifactBundle}
            KindBase::Artifact => {
                match sub_kind {
                    None => {
                        return Err("expected Artifact<kind>".into());
                    }
                    Some(sub_kind) => {
                        return Ok(Self::Artifact(ArtifactSubKind::from_str(sub_kind.as_str())?));
                    }
                };
            }
            KindBase::Proxy => {Self::Proxy}
            KindBase::Credentials => {Self::Credentials}
            KindBase::Control => Self::Control,
            KindBase::UserBase => {
                match sub_kind {
                    None => {
                        return Err("expected UserBase kind (UserBase<kind>)".into());
                    }
                    Some(sub_kind) => {
                        return Ok(Self::UserBase(UserBaseSubKind::from_str(sub_kind.as_str())?));
                    }
                }
            }
        })
    }

    pub fn properties_config(&self) -> &'static PropertiesConfig {
        match self {
            Kind::Space => &UNREQUIRED_BIND_AND_CONFIG_PROERTIES_CONFIG,
            Kind::UserBase(_) => &USER_BASE_PROPERTIES_CONFIG,
            Kind::User => &USER_PROPERTIES_CONFIG,
            Kind::App => &MECHTRON_PROERTIES_CONFIG,
            Kind::Mechtron => &MECHTRON_PROERTIES_CONFIG,
            _ => &DEFAULT_PROPERTIES_CONFIG
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
)]
pub enum DatabaseSubKind {
    Relational(Specific),
}

impl DatabaseSubKind {
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
pub enum BaseSubKind {
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
pub enum UserBaseSubKind {
    Keycloak
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
pub enum FileSubKind {
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
pub enum ArtifactSubKind {
    Raw,
    ParticleConfig,
    Bind,
    Wasm,
    Dir,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Assign {
    pub kind: AssignmentKind,
    pub details: Details,
    pub state: StateSrc,
}


impl Assign {

    pub fn new(kind: AssignmentKind, details: Details, state: StateSrc) -> Self {
        Self {
            kind,
            details,
            state
        }
    }

}

lazy_static! {
    pub static ref DEFAULT_PROPERTIES_CONFIG: PropertiesConfig = default_properties_config();
    pub static ref USER_PROPERTIES_CONFIG: PropertiesConfig = user_properties_config();
    pub static ref USER_BASE_PROPERTIES_CONFIG: PropertiesConfig = userbase_properties_config();
    pub static ref MECHTRON_PROERTIES_CONFIG: PropertiesConfig = mechtron_properties_config();
    pub static ref UNREQUIRED_BIND_AND_CONFIG_PROERTIES_CONFIG: PropertiesConfig = unrequired_bind_and_config_properties_config();
}

fn default_properties_config() -> PropertiesConfig {
    let mut builder = PropertiesConfig::builder();
    builder.build()
}

fn mechtron_properties_config() -> PropertiesConfig {
    let mut builder = PropertiesConfig::builder();
    builder.add("bind", Box::new(PointPattern {}), true, false, PropertySource::Shell, None, false, vec![] );
    builder.add("config", Box::new(PointPattern {}), true, false, PropertySource::Shell, None, false, vec![] );
    builder.build()
}


fn unrequired_bind_and_config_properties_config() -> PropertiesConfig {
    let mut builder = PropertiesConfig::builder();
    builder.add("bind", Box::new(PointPattern {}), false, false, PropertySource::Shell, None, false, vec![] );
    builder.add("config", Box::new(PointPattern {}), false, false, PropertySource::Shell, None, false, vec![] );
    builder.build()
}

fn user_properties_config() -> PropertiesConfig {
    let mut builder = PropertiesConfig::builder();
    builder.add("bind", Box::new(PointPattern {}), true, false, PropertySource::Shell, Some("hyperspace:repo:boot:1.0.0:/bind/user.bind".to_string()), true, vec![] );
    builder.add("username", Box::new(UsernamePattern{}), false, false, PropertySource::Core, None, false, vec![] );
    builder.add("email", Box::new(EmailPattern{}), false, true, PropertySource::Core, None, false, vec![PropertyPermit::Read] );
    builder.add("password", Box::new(AnythingPattern{}), false, true, PropertySource::CoreSecret, None, false, vec![] );
    builder.build()
}

fn userbase_properties_config() -> PropertiesConfig {
    let mut builder = PropertiesConfig::builder();
    builder.add("bind", Box::new(PointPattern {}), true, false, PropertySource::Shell, Some("hyperspace:repo:boot:1.0.0:/bind/userbase.bind".to_string()), true, vec![] );
    builder.add("config", Box::new(PointPattern {}), false, true, PropertySource::Shell, None, false, vec![] );
    builder.add("registration-email-as-username", Box::new(BoolPattern{}), false, false, PropertySource::Shell, Some("true".to_string()), false, vec![] );
    builder.add("verify-email", Box::new(BoolPattern{}), false, false, PropertySource::Shell, Some("false".to_string()), false, vec![] );
    builder.add("sso-session-max-lifespan", Box::new(U64Pattern{}), false, true, PropertySource::Core, Some("315360000".to_string()), false, vec![] );
    builder.build()
}
