#![allow(warnings)]

use async_trait::async_trait;
use cosmic_universe::error::MsgErr;
use cosmic_universe::id::id::{Kind, Layer, Point, Port};
use cosmic_universe::id::Traversal;
use cosmic_universe::sys::{Assign, Sys};
use cosmic_universe::wave::{DirectedHandler, InCtx, ReflectedCore, Router, UltraWave};
use cosmic_universe::State;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, RwLock};

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate async_trait;
