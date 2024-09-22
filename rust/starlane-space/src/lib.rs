#![allow(warnings)]
extern crate alloc;
#[macro_use]
extern crate async_trait;
extern crate core;
#[macro_use]
extern crate enum_ordinalize; //# ! [feature(unboxed_closures)]
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate strum_macros;

use core::str::FromStr;
use std::ops::Deref;

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

use artifact::asynch::ArtifactFetcher;
use command::common::SetProperties;
use command::direct::create::{KindTemplate, Strategy};
use command::direct::delete::Delete;
use command::direct::select::Select;
use config::bind::BindConfig;
use config::Document;
use kind::{ArtifactSubKind, BaseKind, FileSubKind, Kind, Specific, StarSub};
use loc::Surface;
use particle::{Details, Status, Stub};
use point::Point;
use selector::Selector;
use substance::Bin;
use substance::{Substance, ToSubstance};
use wave::core::ReflectedCore;

use crate::err::SpaceErr;
use crate::hyper::ParticleRecord;
use crate::wave::Agent;

pub mod artifact;
pub mod asynch;
pub mod command;
pub mod config;
pub mod err;
pub mod fail;
pub mod frame;
pub mod hyper;
pub mod kind;
pub mod kind2;
pub mod loc;
pub mod log;
pub mod parse;
pub mod particle;
pub mod path;
pub mod point;
pub mod security;
pub mod selector;
pub mod settings;
pub mod substance;
pub mod util;
pub mod wasm;
pub mod wave;

pub static VERSION: Lazy<semver::Version> =
    Lazy::new(|| semver::Version::from_str(include_str!("VERSION").trim()).unwrap());
pub static HYPERUSER: Lazy<Point> =
    Lazy::new(|| Point::from_str("hyperspace:users:hyperuser").expect("point"));
pub static ANONYMOUS: Lazy<Point> =
    Lazy::new(|| Point::from_str("hyperspace:users:anonymous").expect("point"));

/*
pub fn starlane_uuid() -> Uuid {
    uuid::Uuid::new_v4().to_string()
}
pub fn starlane_timestamp() -> DateTime<Utc> {
    Utc::now()
}

 */

#[cfg(test)]
pub mod tests {
    use crate::VERSION;

    #[test]
    fn version() {
        println!("VERSION: {}", VERSION.to_string());
    }
}
