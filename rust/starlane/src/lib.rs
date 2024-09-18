#![allow(warnings)]
#![feature(hasher_prefixfree_extras)]
#[macro_use]
extern crate async_trait;
#[macro_use]
extern crate lazy_static;


#[macro_use]
extern crate starlane_macros;


pub mod err;
pub mod properties;

pub mod env;
#[cfg(feature = "hyperspace")]
pub mod hyper;
mod registry;
#[cfg(feature = "server")]
pub mod server;

#[cfg(feature = "server")]
pub mod host;

pub mod nom;
pub mod cli;


pub fn init()
{
    #[cfg(feature="cli")]
    {
        use rustls::crypto::aws_lc_rs::default_provider;
        default_provider().install_default().expect("crypto provider could not be installed");
    }
}

