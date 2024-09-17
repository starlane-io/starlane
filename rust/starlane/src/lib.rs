#![allow(warnings)]
#[macro_use]
extern crate async_trait;
#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate starlane_macros;

pub mod err;
pub mod properties;

#[cfg(feature = "hyperspace")]
pub mod hyper;
mod registry;
#[cfg(feature = "server")]
pub mod server;

#[cfg(feature = "server")]
pub mod host;

pub mod nom;

pub mod mechtron;
