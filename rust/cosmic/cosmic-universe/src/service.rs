use crate::err::UniErr;
use crate::wave::{Agent, Ping, Pong};

use crate::config::bind::RouteSelector;
use crate::loc::{Point, Topic};
use crate::parse::model::MethodScopeSelector;
use crate::security::Access;
use crate::util::ValueMatcher;
use crate::wave::exchange::DirectedHandler;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, RwLock};
use crate::wave::core::{DirectedCore, Method, ReflectedCore};

/*
#[async_trait]
pub trait Global: Send + Sync {
    async fn handle(&self, request: Ping) -> Pong;
}

 */
