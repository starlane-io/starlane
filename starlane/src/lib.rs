#![allow(warnings)]
//#![feature(hasher_prefixfree_extras)]
#[macro_use]
extern crate async_trait;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate starlane_macros;

extern crate core;

use std::str::FromStr;
use once_cell::sync::Lazy;

pub mod space {
    use chrono::Utc;
    pub use starlane_space::*;
    use starlane_space::wasm::Timestamp;

    #[no_mangle]
    extern "C" fn starlane_uuid() -> loc::Uuid {
        loc::Uuid::from(uuid::Uuid::new_v4().to_string()).unwrap()
    }

    #[no_mangle]
    extern "C" fn starlane_timestamp() -> Timestamp {
        Timestamp::new(Utc::now().timestamp_millis())
    }
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
