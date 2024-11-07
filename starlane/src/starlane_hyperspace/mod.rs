use std::ops::Deref;
use starlane::space::loc::ToBaseKind;
use std::str::FromStr;
use colored::Colorize;
use starlane::space::log::LogAppender;

pub mod layer;
pub mod err;
pub mod global;
pub mod machine;
pub mod reg;
pub mod star;

#[cfg(not(feature="postgres"))]
pub mod tests;

#[cfg(not(feature="postgres"))]
pub mod tests;
pub mod driver;
#[cfg(feature = "hyperlane")]
pub mod hyperlane;
pub mod registry;
pub mod executor;
pub mod host;
pub mod shutdown;
pub mod foundation;
pub mod platform;
pub mod properties;
#[cfg(feature = "service")]
pub mod service;
pub mod template;
pub mod database;

