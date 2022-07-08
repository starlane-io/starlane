#![allow(warnings)]
//# ! [feature(unboxed_closures)]
#[no_std]
#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate strum_macros;
extern crate alloc;
extern crate core;
#[macro_use]
extern crate enum_ordinalize;
#[macro_use]
extern crate async_trait;

use serde::{Deserialize, Serialize};

pub mod error;
pub mod command;
pub mod cli;
pub mod config;
pub mod entity;
pub mod frame;
pub mod http;
pub mod id;
pub mod log;
pub mod msg;
pub mod parse;
pub mod particle;
pub mod portal;
pub mod quota;
pub mod security;
pub mod selector;
pub mod service;
pub mod substance;
pub mod sys;
pub mod util;
pub mod wave;

use crate::error::MsgErr;
use crate::config::config::bind::BindConfig;
use crate::config::config::Document;
use crate::id::id::{Kind, Point, Port, Uuid};
use core::str::FromStr;
use std::ops::Deref;
use std::sync::{Arc, RwLock};
use chrono::{DateTime, Utc};
use dashmap::{DashMap, DashSet};
use std::cmp::Ordering;
use ::http::StatusCode;
use crate::command::command::common::{SetProperties, SetRegistry};
use crate::command::request::delete::Delete;
use crate::command::request::query::{Query, QueryResult};
use crate::command::request::select::{Select, SubSelect};
use crate::particle::particle::{Details, Properties, Status, Stub};
use crate::security::{Access, AccessGrant};
use crate::selector::selector::Selector;
use crate::substance::substance::{Substance, SubstanceList, ToSubstance};
use crate::sys::ParticleRecord;
use crate::wave::{Agent, ReflectedCore};

lazy_static! {
    pub static ref VERSION: semver::Version = semver::Version::from_str("1.0.0").unwrap();
    pub static ref HYPERUSER: Point = Point::from_str("hyperspace:users:hyperuser").expect("point");
    pub static ref ANONYMOUS: Point = Point::from_str("hyperspace:users:anonymous").expect("point");
}


extern "C" {
    pub fn cosmic_uuid() -> Uuid;
    pub fn cosmic_timestamp() -> DateTime<Utc>;
}


#[async_trait]
pub trait ArtifactApi: Send+Sync {
    async fn bind(&self, artifact: &Point) -> Result<ArtRef<BindConfig>, MsgErr>;
}

pub struct ArtRef<A> {
    artifact: Arc<A>,
    bundle: Point,
    point: Point
}

impl <A> ArtRef<A>  {
    pub fn bundle(&self) -> &Point {
        &self.bundle
    }

    pub fn point(&self) -> &Point {
        &self.point
    }
}

impl<A> Deref for ArtRef<A> {
    type Target = Arc<A>;

    fn deref(&self) -> &Self::Target {
        &self.artifact
    }
}

impl<A> Drop for ArtRef<A> {
    fn drop(&mut self) {
        //
    }
}

#[cfg(test)]
pub mod tests {
    #[test]
    fn it_works() {}
}

#[async_trait]
pub trait RegistryApi<E>: Send + Sync where E: CosmicErr {
    async fn register(&self, registration: &Registration) -> Result<Details, E>;

    async fn assign(&self, point: &Point, location: &Point) -> Result<(), E>;

    async fn set_status(&self, point: &Point, status: &Status) -> Result<(), E>;

    async fn set_properties(
        &self,
        point: &Point,
        properties: &SetProperties,
    ) -> Result<(), E>;

    async fn sequence(&self, point: &Point) -> Result<u64, E>;

    async fn get_properties( &self, point:&Point ) -> Result<Properties, E>;

    async fn locate(&self, point: &Point) -> Result<ParticleRecord, E>;

    async fn query(&self, point: &Point, query: &Query) -> Result<QueryResult, E>;

    async fn delete(&self, delete: &Delete ) -> Result<SubstanceList, E>;

    async fn select(&self, select: &mut Select) -> Result<SubstanceList, E>;

    async fn sub_select(&self, sub_select: &SubSelect) -> Result<Vec<Stub>, E>;

    async fn grant(&self, access_grant: &AccessGrant) -> Result<(), E>;

    async fn access(&self, to: &Point, on: &Point) -> Result<Access, E>;

    async fn chown(&self, on: &Selector, owner: &Point, by: &Point) -> Result<(), E>;

    async fn list_access(
        &self,
        to: &Option<&Point>,
        on: &Selector,
    ) -> Result<Vec<IndexedAccessGrant>, E>;

    async fn remove_access(&self, id: i32, to: &Point) -> Result<(), E>;
}

pub trait CosmicErr: Sized+Send+Sync+ToString+Clone{
    fn to_cosmic_err(&self) -> MsgErr;

    fn new<S>(message:S) -> Self where S: ToString;

    fn status_msg<S>(status:u16, message:S) -> Self where S: ToString;

    fn not_found() -> Self {
        Self::not_found_msg("Not Found")
    }

    fn not_found_msg<S>(message:S) -> Self where S: ToString {
        Self::status_msg(404, message )
    }

    fn status(&self) -> u16;

    fn as_reflected_core(&self) -> ReflectedCore {
        let mut core = ReflectedCore::new();
        core.status = StatusCode::from_u16(self.status()).unwrap_or(StatusCode::from_u16(500u16).unwrap());
        core.body = Substance::Empty;
        core
    }
}




pub struct StateCache<C> where C: State {
    pub states: Arc<DashMap<Point,Arc<RwLock<C>>>>
}

impl <C> StateCache<C> where C: State{

}

pub trait StateFactory: Send+Sync{
    fn create(&self) -> Box<dyn State>;
}

pub trait State: Send+Sync{
    fn deserialize<DS>( from: Vec<u8>) -> Result<DS,MsgErr> where DS: State, Self:Sized;
    fn serialize( self ) -> Vec<u8>;
}

pub mod artifact {
    use crate::bin::Bin;
    use crate::id::id::Point;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Artifact {
        pub point: Point,
        pub bin: Bin,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ArtifactRequest {
        pub point: Point,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ArtifactResponse {
        pub to: Point,
        pub payload: Bin,
    }
}

pub mod path {
    use crate::error::MsgErr;
    use crate::parse::consume_path;
    use cosmic_nom::new_span;
    use alloc::format;
    use alloc::string::{String, ToString};
    use alloc::vec::Vec;
    use serde::{Deserialize, Serialize};
    use std::str::FromStr;

    #[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
    pub struct Path {
        string: String,
    }

    impl Path {
        fn new(string: &str) -> Self {
            Path {
                string: string.to_string(),
            }
        }

        pub fn make_absolute(string: &str) -> Result<Self, MsgErr> {
            if string.starts_with("/") {
                Path::from_str(string)
            } else {
                Path::from_str(format!("/{}", string).as_str())
            }
        }

        pub fn bin(&self) -> Result<Vec<u8>, MsgErr> {
            let bin = bincode::serialize(self)?;
            Ok(bin)
        }

        pub fn is_absolute(&self) -> bool {
            self.string.starts_with("/")
        }

        pub fn cat(&self, path: &Path) -> Result<Self, MsgErr> {
            if self.string.ends_with("/") {
                Path::from_str(format!("{}{}", self.string.as_str(), path.string.as_str()).as_str())
            } else {
                Path::from_str(
                    format!("{}/{}", self.string.as_str(), path.string.as_str()).as_str(),
                )
            }
        }

        pub fn parent(&self) -> Option<Path> {
            let s = self.to_string();
            let parent = std::path::Path::new(s.as_str()).parent();
            match parent {
                None => Option::None,
                Some(path) => match path.to_str() {
                    None => Option::None,
                    Some(some) => match Self::from_str(some) {
                        Ok(parent) => Option::Some(parent),
                        Err(error) => {
                            eprintln!("{}", error.to_string());
                            Option::None
                        }
                    },
                },
            }
        }

        pub fn last_segment(&self) -> Option<String> {
            let split = self.string.split("/");
            match split.last() {
                None => Option::None,
                Some(last) => Option::Some(last.to_string()),
            }
        }

        pub fn to_relative(&self) -> String {
            let mut rtn = self.string.clone();
            rtn.remove(0);
            rtn
        }
    }

    impl FromStr for Path {
        type Err = MsgErr;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            let (_, path) = consume_path(new_span(s))?;
            Ok(Self {
                string: path.to_string(),
            })
        }
    }

    impl ToString for Path {
        fn to_string(&self) -> String {
            self.string.clone()
        }
    }
}

pub mod bin {
    use std::collections::HashMap;
    use std::sync::Arc;

    use serde::{Deserialize, Serialize};

    pub type Bin = Arc<Vec<u8>>;
}


pub mod fail {
    use alloc::string::String;
    use serde::{Deserialize, Serialize};

    use crate::error::MsgErr;
    use crate::id::id::Specific;

    pub mod mesh {
        use alloc::string::String;
        use serde::{Deserialize, Serialize};

        #[derive(Debug, Clone, Serialize, Deserialize)]
        pub enum Fail {
            Error(String),
        }
    }

    pub mod portal {
        use alloc::string::String;
        use serde::{Deserialize, Serialize};

        use crate::fail::{http, msg, resource};

        #[derive(Debug, Clone, Serialize, Deserialize)]
        pub enum Fail {
            Error(String),
            Resource(resource::Fail),
            Msg(msg::Fail),
            Http(http::Error),
        }
    }

    pub mod http {
        use alloc::string::String;
        use serde::{Deserialize, Serialize};

        #[derive(Debug, Clone, Serialize, Deserialize)]
        pub struct Error {
            pub message: String,
        }
    }

    pub mod resource {
        use alloc::string::String;
        use serde::{Deserialize, Serialize};

        use crate::fail::{
            Bad, BadCoercion, BadRequest, Conditional, Messaging, NotFound,
        };
        use crate::id::id::Point;

        #[derive(Debug, Clone, Serialize, Deserialize)]
        pub enum Fail {
            Create(Create),
            Update(Update),
            Select(Select),
            BadRequest(BadRequest),
            Conditional(Conditional),
            Messaging(Messaging),
        }

        #[derive(Debug, Clone, Serialize, Deserialize)]
        pub enum Create {
            AddressAlreadyInUse(String),
            WrongParentResourceType { expected: String, found: String },
            CannotUpdateArchetype,
            InvalidProperty { expected: String, found: String },
        }

        #[derive(Debug, Clone, Serialize, Deserialize)]
        pub enum Update {
            Immutable,
        }

        #[derive(Debug, Clone, Serialize, Deserialize)]
        pub enum Select {
            WrongAddress { required: Point, found: Point },
            BadSelectRouting { required: String, found: String },
            BadCoercion(BadCoercion),
        }
    }

    pub mod msg {
        use alloc::string::String;
        use serde::{Deserialize, Serialize};

        use crate::fail::{BadRequest, Conditional};

        #[derive(Debug, Clone, Serialize, Deserialize)]
        pub enum Fail {
            Error(String),
            BadRequest(BadRequest),
            Conditional(Conditional),
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum BadRequest {
        NotFound(NotFound),
        Bad(Bad),
        Illegal(Illegal),
        Wrong(Wrong),
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct BadCoercion {
        pub from: String,
        pub into: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum Conditional {
        Timeout(Timeout),
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Timeout {
        pub waited: i32,
        pub message: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum NotFound {
        Any,
        ResourceType(String),
        Kind(String),
        Specific(String),
        Address(String),
        Key(String),
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum Bad {
        ResourceType(String),
        Kind(String),
        Specific(String),
        Address(String),
        Key(String),
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum Identifier {
        ResourceType,
        Kind,
        Specific,
        Address,
        Key,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum Illegal {
        Immutable,
        EmptyToFieldOnMessage,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Wrong {
        pub received: String,
        pub expected: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum Messaging {
        RequestReplyExchangesRequireOneAndOnlyOneRecipient,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum Fail {
        Mesh(mesh::Fail),
        Resource(resource::Fail),
        Portal(portal::Fail),
        Error(String),
    }

    impl ToString for Fail {
        fn to_string(&self) -> String {
            "Fail".to_string()
        }
    }

    /*    impl Into<MsgErr> for Fail {
           fn into(self) -> MsgErr {
               MsgErr {
                   status: 500,
                   message: "Fail".to_string(),
               }
           }
       }

    */
}

#[derive(Clone)]
pub struct Registration {
    pub point: Point,
    pub kind: Kind,
    pub registry: SetRegistry,
    pub properties: SetProperties,
    pub owner: Point,
}


#[derive(Clone)]
pub enum MountKind{
    Control,
    Portal
}

impl MountKind {
    pub fn kind(&self) -> Kind {
        match self {
            MountKind::Control => Kind::Control,
            MountKind::Portal => Kind::Portal
        }
    }
}

#[derive(Debug, Clone)]
pub struct IndexedAccessGrant {
    pub id: i32,
    pub access_grant: AccessGrant,
}

impl Eq for IndexedAccessGrant {}

impl PartialEq<Self> for IndexedAccessGrant {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Ord for IndexedAccessGrant {
    fn cmp(&self, other: &Self) -> Ordering {
        if self.id < other.id {
            Ordering::Greater
        } else if self.id < other.id {
            Ordering::Less
        } else {
            Ordering::Equal
        }
    }
}

impl PartialOrd<Self> for IndexedAccessGrant {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self.id < other.id {
            Some(Ordering::Greater)
        } else if self.id < other.id {
            Some(Ordering::Less)
        } else {
            Some(Ordering::Equal)
        }
    }
}

impl Deref for IndexedAccessGrant {
    type Target = AccessGrant;

    fn deref(&self) -> &Self::Target {
        &self.access_grant
    }
}

impl Into<AccessGrant> for IndexedAccessGrant {
    fn into(self) -> AccessGrant {
        self.access_grant
    }
}
