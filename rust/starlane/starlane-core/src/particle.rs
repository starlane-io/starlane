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

use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot};
use tokio::sync::oneshot::Receiver;
use tracing_futures::WithSubscriber;

use cosmic_nom::new_span;
use cosmic_universe::hyper::{AssignmentKind, ChildRegistry, Location};
use cosmic_universe::id2::BaseSubKind;
use cosmic_universe::kind::{ArtifactSubKind, BaseKind, FileSubKind, UserBaseSubKind};
use cosmic_universe::loc::{StarKey, ToPoint};
use cosmic_universe::loc::ToBaseKind;
use cosmic_universe::parse::{CamelCase, consume_kind};
use cosmic_universe::particle::{Details, Property};
use mesh_portal::error::MsgErr;
use mesh_portal::version::latest::command::common::StateSrc;
use mesh_portal::version::latest::config::{ParticleConfigBody, PointConfig};
use mesh_portal::version::latest::entity::request::create::KindTemplate;
use mesh_portal::version::latest::id::{KindParts, Point, ResourceKind, Specific};
use mesh_portal::version::latest::particle::{Status, Stub};
use mesh_portal::version::latest::payload::Substance;
use mesh_portal::version::latest::security::Permissions;

use crate::{error, logger, util};
use crate::config::config::ParticleConfig;
use crate::error::Error;
use crate::fail::Fail;
use crate::file_access::FileAccess;
use crate::frame::{ResourceHostAction, StarMessagePayload};
use crate::logger::{elog, LogInfo, StaticLogInfo};
use crate::message::{MessageExpect, ProtoStarMessage, ReplyKind};
use crate::names::Name;
use crate::particle::property::{
    AnythingPattern, BoolPattern, EmailPattern, PointPattern, PropertiesConfig, PropertyPermit,
    PropertySource, U64Pattern,
};
use crate::star::{StarInfo, StarSkel};
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
    Location::new(StarKey::central().to_point())
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

/*
impl TryInto<KindTemplate> for Kind {
    type Error = mesh_portal::error::MsgErr;

    fn try_into(self) -> Result<KindTemplate, Self::Error> {
        Ok(KindTemplate {
            kind: self.base().into(),
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

 */

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

/*
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
            base: self.base().into(),
            sub: self.sub_kind(),
            specific: self.specific()
        }
    }
}

impl Into<KindBase> for Kind {
    fn into(self) -> KindBase {
        self.base().into()
    }
}

impl Into<KindBase> for KindBase {
    fn into(self) -> KindBase {
        match self {
            KindBase::Root => KindBase::Root,
            KindBase::Space => KindBase::Ext(CamelCase::from_str("Space").unwrap()),
            KindBase::UserBase => KindBase::Ext(CamelCase::from_str("UserBase").unwrap()),
            KindBase::Base => KindBase::Ext(CamelCase::from_str("Base").unwrap()),
            KindBase::User => KindBase::Ext(CamelCase::from_str("User").unwrap()),
            KindBase::App => KindBase::Ext(CamelCase::from_str("App").unwrap()),
            KindBase::Mechtron => KindBase::Ext(CamelCase::from_str("Mechtron").unwrap()),
            KindBase::FileSystem => KindBase::Ext(CamelCase::from_str("FileSystem").unwrap()),
            KindBase::File => KindBase::Ext(CamelCase::from_str("File").unwrap()),
            KindBase::Database => KindBase::Ext(CamelCase::from_str("Database").unwrap()),
            KindBase::Authenticator => KindBase::Ext(CamelCase::from_str("Authenticator").unwrap()),
            KindBase::BundleSeries => KindBase::BundleSeries,
            KindBase::Bundle =>  KindBase::Bundle,
            KindBase::Artifact => KindBase::Ext(CamelCase::from_str("Artifact").unwrap()),
            KindBase::Control => KindBase::Ext(CamelCase::from_str("Control").unwrap()),
            KindBase::Proxy => KindBase::Ext(CamelCase::from_str("Proxy").unwrap()),
            KindBase::Credentials => KindBase::Ext(CamelCase::from_str("Credentials").unwrap()),
        }
    }
}

impl Kind {
    pub fn base(&self) -> KindBase {
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
            Kind::BundleSeries => KindBase::BundleSeries,
            Kind::Bundle => KindBase::Bundle,
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
            KindBase::BundleSeries => {Self::BundleSeries }
            KindBase::Bundle => {Self::Bundle }
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

}

 */

/*
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Assign {
    pub kind: AssignmentKind,
    pub details: Details,
    pub state: StateSrc,
}


impl Assign {

    pub fn new( kind: AssignmentKind,
                details: Details,
                state: StateSrc,

    ) -> Self {
        Self {
            kind,
            details,
            state,
        }
    }

    pub fn config(&self) -> Option<Result<Point,MsgErr>> {
        match self.details.properties.get("config") {
            None => None,
            Some(prop) => {
                Some(Point::from_str(prop.value.as_str() ))
            }
        }
    }

}


 */
lazy_static! {
    pub static ref DEFAULT_PROPERTIES_CONFIG: PropertiesConfig = default_properties_config();
    pub static ref USER_PROPERTIES_CONFIG: PropertiesConfig = user_properties_config();
    pub static ref USER_BASE_PROPERTIES_CONFIG: PropertiesConfig = userbase_properties_config();
    pub static ref MECHTRON_PROERTIES_CONFIG: PropertiesConfig = mechtron_properties_config();
    pub static ref UNREQUIRED_BIND_AND_CONFIG_PROERTIES_CONFIG: PropertiesConfig =
        unrequired_bind_and_config_properties_config();
}

fn default_properties_config() -> PropertiesConfig {
    let mut builder = PropertiesConfig::builder();
    builder.build()
}

fn mechtron_properties_config() -> PropertiesConfig {
    let mut builder = PropertiesConfig::builder();
    builder.add(
        "bind",
        Box::new(PointPattern {}),
        true,
        false,
        PropertySource::Shell,
        None,
        false,
        vec![],
    );
    builder.add(
        "config",
        Box::new(PointPattern {}),
        true,
        false,
        PropertySource::Shell,
        None,
        false,
        vec![],
    );
    builder.build()
}

fn unrequired_bind_and_config_properties_config() -> PropertiesConfig {
    let mut builder = PropertiesConfig::builder();
    builder.add(
        "bind",
        Box::new(PointPattern {}),
        false,
        false,
        PropertySource::Shell,
        None,
        false,
        vec![],
    );
    builder.add(
        "config",
        Box::new(PointPattern {}),
        false,
        false,
        PropertySource::Shell,
        None,
        false,
        vec![],
    );
    builder.build()
}

fn user_properties_config() -> PropertiesConfig {
    let mut builder = PropertiesConfig::builder();
    builder.add(
        "bind",
        Box::new(PointPattern {}),
        true,
        false,
        PropertySource::Shell,
        Some("hyperspace:repo:boot:1.0.0:/bind/user.bind".to_string()),
        true,
        vec![],
    );
    builder.add(
        "username",
        Box::new(UsernamePattern {}),
        false,
        false,
        PropertySource::Core,
        None,
        false,
        vec![],
    );
    builder.add(
        "email",
        Box::new(EmailPattern {}),
        false,
        true,
        PropertySource::Core,
        None,
        false,
        vec![PropertyPermit::Read],
    );
    builder.add(
        "password",
        Box::new(AnythingPattern {}),
        false,
        true,
        PropertySource::CoreSecret,
        None,
        false,
        vec![],
    );
    builder.build()
}

fn userbase_properties_config() -> PropertiesConfig {
    let mut builder = PropertiesConfig::builder();
    builder.add(
        "bind",
        Box::new(PointPattern {}),
        true,
        false,
        PropertySource::Shell,
        Some("hyperspace:repo:boot:1.0.0:/bind/userbase.bind".to_string()),
        true,
        vec![],
    );
    builder.add(
        "config",
        Box::new(PointPattern {}),
        false,
        true,
        PropertySource::Shell,
        None,
        false,
        vec![],
    );
    builder.add(
        "registration-email-as-username",
        Box::new(BoolPattern {}),
        false,
        false,
        PropertySource::Shell,
        Some("true".to_string()),
        false,
        vec![],
    );
    builder.add(
        "verify-email",
        Box::new(BoolPattern {}),
        false,
        false,
        PropertySource::Shell,
        Some("false".to_string()),
        false,
        vec![],
    );
    builder.add(
        "sso-session-max-lifespan",
        Box::new(U64Pattern {}),
        false,
        true,
        PropertySource::Core,
        Some("315360000".to_string()),
        false,
        vec![],
    );
    builder.build()
}

pub fn properties_config<K: ToBaseKind>(base: &K) -> &'static PropertiesConfig {
    match base.to_base() {
        BaseKind::Space => &UNREQUIRED_BIND_AND_CONFIG_PROERTIES_CONFIG,
        BaseKind::UserBase => &USER_BASE_PROPERTIES_CONFIG,
        BaseKind::User => &USER_PROPERTIES_CONFIG,
        BaseKind::App => &MECHTRON_PROERTIES_CONFIG,
        BaseKind::Mechtron => &MECHTRON_PROERTIES_CONFIG,
        _ => &DEFAULT_PROPERTIES_CONFIG,
    }
}
