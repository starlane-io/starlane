use crate::error::MsgErr;
use crate::wave::{Agent, Method, Ping, DirectedCore, Pong, ReflectedCore};
use crate::id::id::{Point, Topic};

use crate::parse::model::MethodScopeSelector;
use crate::security::Access;
use crate::util::ValueMatcher;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, RwLock};
use crate::config::config::bind::RouteSelector;
use crate::wave::DirectedHandler;

pub trait AccessProvider: Send + Sync {
    fn access(&self, to: &Agent, on: &Point) -> Result<Access, MsgErr>;
}

pub struct AllAccessProvider();

impl AccessProvider for AllAccessProvider {
    fn access(&self, _: &Agent, _: &Point) -> Result<Access, MsgErr> {
        Ok(Access::SuperOwner)
    }
}

#[async_trait]
pub trait Global: Send + Sync {
    async fn handle(&self, request: Ping) -> Pong;
}



