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
use crate::version::v0_0_1::id::id::{Point, Port, Uuid};
use core::str::FromStr;
use std::ops::Deref;
use std::sync::{Arc, RwLock};
use dashmap::{DashMap, DashSet};
use crate::version::v0_0_1::security::Access;
use crate::version::v0_0_1::substance::substance::Substance;
use crate::version::v0_0_1::sys::ParticleRecord;
use crate::version::v0_0_1::wave::Agent;

lazy_static! {
    pub static ref VERSION: semver::Version = semver::Version::from_str("1.0.0").unwrap();
}

#[async_trait]
pub trait Artifacts: Send+Sync {
    async fn bind(&self, artifact: &Point) -> Result<ArtRef<BindConfig>, MsgErr>;
}

pub struct ArtRef<A> {
    artifact: Arc<A>,
    bundle: Point,
    point: Point
}

impl <A> ArtRef<A>  {
    pub fn bundle(&self) -> &Point {
        &self.bundle
    }

    pub fn point(&self) -> &Point {
        &self.point
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

#[async_trait]
pub trait RegistryApi: Send + Sync {
    async fn access(&self, to: &Agent, on: &Point) -> Result<Access,MsgErr>;
    async fn locate(&self, particle: &Point) -> Result<ParticleRecord,MsgErr>;
}

pub struct StateCache<C> where C: State {
    pub states: Arc<DashMap<Point,Arc<RwLock<C>>>>
}

impl <C> StateCache<C> where C: State{

}

pub trait StateFactory: Send+Sync{
    fn create(&self) -> Box<dyn State>;
}

pub trait State: Send+Sync {
    fn deserialize<DS>( from: Vec<u8>) -> Result<DS,MsgErr> where DS: State, Self:Sized;
    fn serialize( self ) -> Vec<u8>;
}
