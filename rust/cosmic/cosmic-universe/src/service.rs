use crate::error::UniErr;
use crate::wave::{Agent, DirectedCore, Method, Ping, Pong, ReflectedCore};

use crate::config::bind::RouteSelector;
use crate::loc::{Point, Topic};
use crate::parse::model::MethodScopeSelector;
use crate::security::Access;
use crate::util::ValueMatcher;
use crate::wave::DirectedHandler;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, RwLock};

/*
#[async_trait]
pub trait Global: Send + Sync {
    async fn handle(&self, request: Ping) -> Pong;
}

 */
