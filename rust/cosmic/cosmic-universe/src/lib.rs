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
pub mod error;
pub mod frame2;
pub mod http;
pub mod id2;
pub mod log;
pub mod ext;
pub mod parse;
pub mod particle2;
pub mod property;
pub mod quota;
pub mod security;
pub mod selector2;
pub mod service;
pub mod substance2;
pub mod hyper;
pub mod util;
pub mod wave;
pub mod artifact;
pub mod state;
pub mod path;
pub mod fail;
pub mod reg;
pub mod mount;
pub mod command;
pub mod config;


use substance2::Bin;
use command::common::{SetProperties, SetRegistry};
use command::request::create::{KindTemplate, Strategy};
use command::request::delete::Delete;
use command::request::query::{Query, QueryResult};
use command::request::select::{Select, SubSelect};
use config::bind::BindConfig;
use config::Document;
use crate::error::UniErr;
use crate::id2::id::{BaseKind, Kind, Point, Port, Specific, Uuid};
use crate::id2::{ArtifactSubKind, FileSubKind, StarSub, UserBaseSubKind};
use crate::particle2::particle::{Details, Properties, Status, Stub};
use crate::security::{Access, AccessGrant};
use crate::selector2::selector::Selector;
use crate::substance2::substance::{Substance, SubstanceList, Token, ToSubstance};
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
