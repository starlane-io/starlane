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

pub mod error;
pub mod http;
pub mod log;
pub mod ext;
pub mod parse;
pub mod property;
pub mod quota;
pub mod security;
pub mod service;
pub mod hyper;
pub mod util;
pub mod wave;
pub mod artifact;
pub mod path;
pub mod fail;
pub mod reg;
pub mod mount;
pub mod command;
pub mod config;
pub mod id;
pub mod particle;
pub mod frame;
pub mod selector;
pub mod substance;
pub mod kind;


use substance::Bin;
use command::common::{SetProperties, SetRegistry};
use command::direct::create::{KindTemplate, Strategy};
use command::direct::delete::Delete;
use command::direct::query::{Query, QueryResult};
use command::direct::select::{Select, SubSelect};
use config::bind::BindConfig;
use config::Document;
use crate::error::UniErr;
use crate::security::{Access, AccessGrant};
use selector::Selector;
use crate::hyper::ParticleRecord;
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
use artifact::ArtifactFetcher;
use id::{ArtifactSubKind, BaseKind, FileSubKind, Kind, Point, Port, Specific, StarSub, UserBaseSubKind, Uuid};
use particle::{Details, Properties, Status, Stub};
use substance::{Substance, SubstanceList, Token, ToSubstance};

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

#[cfg(test)]
pub mod tests {
    #[test]
    fn it_works() {}
}
