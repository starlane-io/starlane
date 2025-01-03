#![allow(warnings)]

#[macro_use]
extern crate async_trait;
extern crate core;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate starlane_macros;

pub extern crate starlane_macros as macros;
pub extern crate starlane_space as space;
pub extern crate starlane_hyperspace as hyperspace;
pub extern crate starlane_base as base;

shadow!(build);

use once_cell::sync::Lazy;
use shadow_rs::shadow;
use std::str::FromStr;

pub static VERSION: Lazy<semver::Version> =
    Lazy::new(|| semver::Version::from_str(env!("CARGO_PKG_VERSION").trim()).unwrap());

pub fn init() {

    {
        use rustls::crypto::aws_lc_rs::default_provider;
        default_provider()
            .install_default()
            .expect("crypto provider could not be installed");
    }
}
