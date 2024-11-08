#![allow(warnings)]
/*
#![feature(prelude_import)]
#![feature(custom_inner_attributes)]
#![feature(proc_macro_hygiene)]

 */
//#![starlane_primitive_macros::loggerhead]
//extern crate alloc;
#[macro_use]
extern crate async_trait;
#[macro_use]
extern crate enum_ordinalize; //# ! [feature(unboxed_closures)]
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate strum_macros;

extern crate core;

use std::ops::Deref;

use serde::{Deserialize, Serialize};

use crate::artifact::asynch::ArtifactFetcher;
use crate::command::common::SetProperties;
use crate::command::direct::create::KindTemplate;
use crate::command::direct::delete::Delete;
use crate::command::direct::select::Select;
use crate::config::bind::BindConfig;
use crate::kind::{BaseKind, Kind, StarSub};
use crate::loc::Surface;
use crate::particle::{Details, Status, Stub};
use crate::substance::Bin;
use crate::substance::{Substance, ToSubstance};
use crate::wave::core::ReflectedCore;

use crate::err::SpaceErr;
use crate::hyper::ParticleRecord;
use crate::wave::Agent;

pub(crate) extern crate self as starlane_space;

/*
pub fn starlane_uuid() -> Uuid {
    uuid::Uuid::new_v4().to_string()
}
pub fn starlane_timestamp() -> DateTime<Utc> {
    Utc::now()
}

 */

use crate::point::Point;
use core::str::FromStr;
use once_cell::sync::Lazy;

pub mod artifact;
pub mod asynch;
pub mod command;
pub mod config;
pub mod err;
pub mod fail;
pub mod frame;
pub mod hyper;
pub mod kind;
pub mod parse;
pub mod particle;
pub mod wave;

#[cfg(feature = "kind2")]
pub mod kind2;

pub mod loc;
pub mod log;
pub mod path;
pub mod point;
pub mod security;
pub mod selector;
pub mod settings;
pub mod substance;
pub mod util;
pub mod wasm;

pub mod prelude;
pub mod task;

pub static VERSION: Lazy<semver::Version> =
    Lazy::new(|| semver::Version::from_str(env!("CARGO_PKG_VERSION").trim()).unwrap());

pub static HYPERUSER: Lazy<Point> =
    Lazy::new(|| Point::from_str("hyperspace:users:hyperuser").expect("point"));
pub static ANONYMOUS: Lazy<Point> =
    Lazy::new(|| Point::from_str("hyperspace:users:anonymous").expect("point"));

#[cfg(test)]
pub mod test {
    use crate::VERSION;

    #[test]
    pub fn test_version() {
        println!("{}", VERSION.to_string());
    }
}
