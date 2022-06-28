use crate::error::MsgErr;
use crate::version::v0_0_1::wave::{Agent, Method, ReqShell, ReqCore, RespShell, RespCore};
use crate::version::v0_0_1::id::id::{Point, Topic};

use crate::version::v0_0_1::parse::model::MethodScopeSelector;
use crate::version::v0_0_1::security::Access;
use crate::version::v0_0_1::util::ValueMatcher;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, RwLock};
use crate::version::v0_0_1::config::config::bind::RouteSelector;
use crate::version::v0_0_1::wave::RequestHandler;

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
    async fn handle(&self, request: ReqShell) -> RespShell;
}



