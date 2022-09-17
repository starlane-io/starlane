use crate::error::UniErr;
use crate::id::id::{Point, Topic};
use crate::wave::{Agent, DirectedCore, Method, Ping, Pong, ReflectedCore};

use crate::config::config::bind::RouteSelector;
use crate::parse::model::MethodScopeSelector;
use crate::security::Access;
use crate::util::ValueMatcher;
use crate::wave::DirectedHandler;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, RwLock};

pub trait AccessProvider: Send + Sync {
    fn access(&self, to: &Agent, on: &Point) -> Result<Access, UniErr>;
}

pub struct AllAccessProvider();

impl AccessProvider for AllAccessProvider {
    fn access(&self, _: &Agent, _: &Point) -> Result<Access, UniErr> {
        Ok(Access::SuperOwner)
    }
}

#[async_trait]
pub trait Global: Send + Sync {
    async fn handle(&self, request: Ping) -> Pong;
}
