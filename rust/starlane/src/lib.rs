#![allow(warnings)]
//#![feature(hasher_prefixfree_extras)]
#[macro_use]
extern crate async_trait;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate starlane_macros;

#[cfg(feature="space")]
pub extern crate starlane_space as starlane;
extern crate core;

#[cfg(test)]
pub mod test;

pub mod foundation;

pub mod shutdown;

#[cfg(feature="space")]
pub mod space {
    pub use starlane_space::space::*;
}

pub mod template;


//pub(crate) use starlane_space as starlane;

pub mod err;
pub mod properties;

pub mod env;

#[cfg(feature = "hyperspace")]
pub mod hyperspace;

#[cfg(feature = "hyperlane")]
pub mod hyperlane;

//pub mod space;

pub mod registry;
#[cfg(feature = "server")]
pub mod server;

use std::str::FromStr;
use once_cell::sync::Lazy;
#[cfg(feature = "server")]
pub use server::*;

pub mod host;

pub mod cli;

//pub mod store;

pub mod driver;


pub mod executor;

pub mod platform;

#[cfg(feature = "service")]
pub mod service;





pub static VERSION: Lazy<semver::Version> =
    Lazy::new(|| semver::Version::from_str(env!("CARGO_PKG_VERSION").trim()).unwrap() );

pub fn init() {
    #[cfg(feature = "cli")]
    {
        use rustls::crypto::aws_lc_rs::default_provider;
        default_provider()
            .install_default()
            .expect("crypto provider could not be installed");
    }
}
