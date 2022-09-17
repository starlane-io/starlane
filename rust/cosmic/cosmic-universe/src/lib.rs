#![allow(warnings)]
//# ! [feature(unboxed_closures)]
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

pub mod cli;
pub mod command;
pub mod config;
pub mod error;
pub mod frame;
pub mod http;
pub mod id;
pub mod log;
pub mod ext;
pub mod parse;
pub mod particle;
pub mod portal;
pub mod property;
pub mod quota;
pub mod security;
pub mod selector;
pub mod service;
pub mod substance;
pub mod sys;
pub mod util;
pub mod wave;

use crate::bin::Bin;
use crate::command::command::common::{SetProperties, SetRegistry};
use crate::command::request::create::{KindTemplate, Strategy};
use crate::command::request::delete::Delete;
use crate::command::request::query::{Query, QueryResult};
use crate::command::request::select::{Select, SubSelect};
use crate::config::config::bind::BindConfig;
use crate::config::config::Document;
use crate::error::UniErr;
use crate::id::id::{BaseKind, Kind, Point, Port, Specific, Uuid};
use crate::id::{ArtifactSubKind, FileSubKind, StarSub, UserBaseSubKind};
use crate::particle::particle::{Details, Properties, Status, Stub};
use crate::security::{Access, AccessGrant};
use crate::selector::selector::Selector;
use crate::substance::substance::{Substance, SubstanceList, ToSubstance, Token};
use crate::sys::ParticleRecord;
use crate::wave::{Agent, ReflectedCore};
use ::http::StatusCode;
use chrono::{DateTime, Utc};
use core::str::FromStr;
use dashmap::{DashMap, DashSet};
use lru::LruCache;
use std::cmp::Ordering;
use std::ops::Deref;
use std::sync::Arc;
use tokio::sync::RwLock;

lazy_static! {
    pub static ref VERSION: semver::Version = semver::Version::from_str("0.3.0").unwrap();
    pub static ref HYPERUSER: Point = Point::from_str("hyperspace:users:hyperuser").expect("point");
    pub static ref ANONYMOUS: Point = Point::from_str("hyperspace:users:anonymous").expect("point");
}

pub fn cosmic_uuid() -> Uuid {
    uuid::Uuid::new_v4().to_string()
}
pub fn cosmic_timestamp() -> DateTime<Utc> {
    Utc::now()
}

#[derive(Clone)]
pub struct ArtifactApi {
    binds: Arc<RwLock<LruCache<Point, Arc<BindConfig>>>>,
    fetcher: Arc<dyn ArtifactFetcher>,
}

impl ArtifactApi {
    pub fn new(fetcher: Arc<dyn ArtifactFetcher>) -> Self {
        Self {
            binds: Arc::new(RwLock::new(LruCache::new(1024))),
            fetcher,
        }
    }

    pub async fn bind(&self, point: &Point) -> Result<ArtRef<BindConfig>, UniErr> {
        {
            let read = self.binds.read().await;
            if read.contains(point) {
                let mut write = self.binds.write().await;
                let bind = write.get(point).unwrap().clone();
                return Ok(ArtRef::new(bind, point.clone()));
            }
        }

        let bind: Arc<BindConfig> = Arc::new(self.get(point).await?);
        {
            let mut write = self.binds.write().await;
            write.put(point.clone(), bind.clone());
        }
        return Ok(ArtRef::new(bind, point.clone()));
    }

    async fn get<A>(&self, point: &Point) -> Result<A, UniErr>
    where
        A: TryFrom<Vec<u8>, Error =UniErr>,
    {
        if !point.has_bundle() {
            return Err("point is not from a bundle".into());
        }
        let bin = self.fetcher.fetch(point).await?;
        Ok(A::try_from(bin)?)
    }
}

pub struct NoDiceArtifactFetcher {}

impl NoDiceArtifactFetcher {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl ArtifactFetcher for NoDiceArtifactFetcher {
    async fn stub(&self, point: &Point) -> Result<Stub, UniErr> {
        Err(UniErr::from_status(404u16))
    }

    async fn fetch(&self, point: &Point) -> Result<Vec<u8>, UniErr> {
        Err(UniErr::from_status(404u16))
    }
}

#[derive(Clone)]
pub struct ArtRef<A> {
    artifact: Arc<A>,
    point: Point,
}

impl<A> ArtRef<A> {
    pub fn new(artifact: Arc<A>, point: Point) -> Self {
        Self { artifact, point }
    }
}

impl<A> ArtRef<A> {
    pub fn bundle(&self) -> Point {
        self.point.clone().to_bundle().unwrap()
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

#[async_trait]
pub trait ArtifactFetcher: Send + Sync {
    async fn stub(&self, point: &Point) -> Result<Stub, UniErr>;
    async fn fetch(&self, point: &Point) -> Result<Vec<u8>, UniErr>;
}

pub struct FetchErr {}

#[cfg(test)]
pub mod tests {
    #[test]
    fn it_works() {}
}

pub struct StateCache<C>
where
    C: State,
{
    pub states: Arc<DashMap<Point, Arc<RwLock<C>>>>,
}

impl<C> StateCache<C> where C: State {}

pub trait StateFactory: Send + Sync {
    fn create(&self) -> Box<dyn State>;
}

pub trait State: Send + Sync {
    fn deserialize<DS>(from: Vec<u8>) -> Result<DS, UniErr>
    where
        DS: State,
        Self: Sized;
    fn serialize(self) -> Vec<u8>;
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
    use crate::error::UniErr;
    use crate::parse::consume_path;
    use alloc::format;
    use alloc::string::{String, ToString};
    use alloc::vec::Vec;
    use cosmic_nom::new_span;
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

        pub fn make_absolute(string: &str) -> Result<Self, UniErr> {
            if string.starts_with("/") {
                Path::from_str(string)
            } else {
                Path::from_str(format!("/{}", string).as_str())
            }
        }

        pub fn bin(&self) -> Result<Vec<u8>, UniErr> {
            let bin = bincode::serialize(self)?;
            Ok(bin)
        }

        pub fn is_absolute(&self) -> bool {
            self.string.starts_with("/")
        }

        pub fn cat(&self, path: &Path) -> Result<Self, UniErr> {
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
        type Err = UniErr;

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

    use crate::error::UniErr;
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

        use crate::fail::{http, ext, resource};

        #[derive(Debug, Clone, Serialize, Deserialize)]
        pub enum Fail {
            Error(String),
            Resource(resource::Fail),
            Ext(ext::Fail),
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

        use crate::fail::{Bad, BadCoercion, BadRequest, Conditional, Messaging, NotFound};
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

    pub mod ext {
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

    /*    impl Into<ExtErr> for Fail {
           fn into(self) -> ExtErr {
               ExtErr {
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
    pub strategy: Strategy,
    pub status: Status
}

#[derive(Clone)]
pub enum MountKind {
    Control,
    Portal,
}

impl MountKind {
    pub fn kind(&self) -> Kind {
        match self {
            MountKind::Control => Kind::Control,
            MountKind::Portal => Kind::Portal,
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
