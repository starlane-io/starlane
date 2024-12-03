#![allow(warnings)]

shadow!(build);

//#![feature(hasher_prefixfree_extras)]
#[macro_use]
extern crate async_trait;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate starlane_macros;

extern crate core;

use once_cell::sync::Lazy;
use shadow_rs::shadow;
use std::str::FromStr;

pub mod space;

pub mod hyperspace;

#[cfg(feature = "server")]
pub mod env;

#[cfg(feature = "server")]
pub mod server;

pub static VERSION: Lazy<semver::Version> =
    Lazy::new(|| semver::Version::from_str(env!("CARGO_PKG_VERSION").trim()).unwrap());

pub fn init() {
    #[cfg(feature = "cli")]
    {
        use rustls::crypto::aws_lc_rs::default_provider;
        default_provider()
            .install_default()
            .expect("crypto provider could not be installed");
    }
}
