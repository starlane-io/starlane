#![allow(warnings)]
//#![feature(hasher_prefixfree_extras)]
#[macro_use]
extern crate async_trait;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate starlane_macros;

pub extern crate starlane_space as starlane;
extern crate core;

use std::str::FromStr;
use once_cell::sync::Lazy;

pub mod space {
    pub use starlane_space::space::*;
}


pub mod env;

pub mod server;

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
