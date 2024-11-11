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
use std::str::FromStr;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

use crate::space::artifact::asynch::ArtifactFetcher;
use crate::space::command::common::SetProperties;
use crate::space::command::direct::create::KindTemplate;
use crate::space::command::direct::delete::Delete;
use crate::space::command::direct::select::Select;
use crate::space::config::bind::BindConfig;
use crate::space::kind::{BaseKind, Kind, StarSub};
use crate::space::loc::Surface;
use crate::space::particle::{Details, Status, Stub};
use crate::space::substance::Bin;
use crate::space::substance::{Substance, ToSubstance};
use crate::space::wave::core::ReflectedCore;

use crate::space::err::SpaceErr;
use crate::space::hyper::ParticleRecord;
use crate::space::point::Point;
use crate::space::wave::Agent;


/*
pub fn starlane_uuid() -> Uuid {
    uuid::Uuid::new_v4().to_string()
}
pub fn starlane_timestamp() -> DateTime<Utc> {
    Utc::now()
}

 */


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
    use crate::space::VERSION;

    #[test]
    pub fn test_version() {
        println!("{}", VERSION.to_string());
    }
}
