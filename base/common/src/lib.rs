#![allow(warnings)]

//#![feature(hasher_prefixfree_extras)]
use once_cell::sync::Lazy;
use shadow_rs::shadow;
use std::str::FromStr;

pub mod base;

#[cfg(test)]
pub mod test;

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
