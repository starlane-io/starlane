#![allow(warnings)]

use async_trait::async_trait;
use cosmic_api::error::MsgErr;
use cosmic_api::id::id::{Kind, Layer, Point, Port};
use cosmic_api::State;
use cosmic_api::wave::{DirectedHandler, InCtx, ReflectedCore, Router, UltraWave};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, RwLock};
use cosmic_api::id::Traversal;
use cosmic_api::sys::{Assign, Sys};

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate async_trait;