#![allow(warnings)]
#![feature(integer_atomics)]
//# ! [feature(unboxed_closures)]
#[no_std]
#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate strum_macros;
extern crate alloc;
extern crate core;
#[macro_use]
extern crate enum_ordinalize;
#[macro_use]
extern crate async_trait;

use serde::{Deserialize, Serialize};

pub mod error;
pub mod version;

use crate::error::MsgErr;
use crate::version::v0_0_1::config::config::bind::BindConfig;
use crate::version::v0_0_1::config::config::Document;
use crate::version::v0_0_1::id::id::Point;
use core::str::FromStr;
use std::ops::Deref;
use std::sync::Arc;

lazy_static! {
    pub static ref VERSION: semver::Version = semver::Version::from_str("1.0.0").unwrap();
}

#[async_trait]
pub trait Artifacts: Send+Sync {
    async fn bind(&self, point: &Point) -> Result<ArtRef<BindConfig>, MsgErr>;
}

pub struct ArtRef<A> {
    artifact: Arc<A>,
    bundle: Point
}

impl <A> ArtRef<A>  {
    pub fn bundle(&self) -> &Point {
        &self.bundle
    }
}

impl<A> Deref for ArtRef<A> {
    type Target = Arc<A>;

    fn deref(&self) -> &Self::Target {
        &self.artifact
    }
}

impl<A> Drop for ArtRef<A> {
    fn drop(&mut self) {
        //
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {}
}
