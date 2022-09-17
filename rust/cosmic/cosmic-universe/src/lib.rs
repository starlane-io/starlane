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

pub mod artifact;
pub mod command;
pub mod config;
pub mod err;
pub mod fail;
pub mod frame;
pub mod hyper;
pub mod loc;
pub mod kind;
pub mod log;
pub mod parse;
pub mod particle;
pub mod path;
pub mod settings;
pub mod security;
pub mod selector;
pub mod substance;
pub mod util;
pub mod wave;

use crate::err::UniErr;
use crate::hyper::ParticleRecord;
use crate::security::{Access, AccessGrant};
use crate::wave::Agent;
use ::http::StatusCode;
use artifact::ArtifactFetcher;
use chrono::{DateTime, Utc};
use command::common::{SetProperties, SetRegistry};
use command::direct::create::{KindTemplate, Strategy};
use command::direct::delete::Delete;
use command::direct::query::{Query, QueryResult};
use command::direct::select::{Select, SubSelect};
use config::bind::BindConfig;
use config::Document;
use core::str::FromStr;
use dashmap::{DashMap, DashSet};
use loc::{
    Point, Specific, Surface,
    Uuid,
};
use lru::LruCache;
use particle::{Details, Properties, Status, Stub};
use selector::Selector;
use std::cmp::Ordering;
use std::ops::Deref;
use std::sync::Arc;
use substance::Bin;
use substance::{Substance, SubstanceList, Token, ToSubstance};
use tokio::sync::RwLock;
use kind::{ArtifactSubKind, BaseKind, FileSubKind, Kind, StarSub, UserBaseSubKind};
use wave::core::ReflectedCore;

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
