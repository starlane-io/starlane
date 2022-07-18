#![allow(warnings)]

use async_trait::async_trait;
use cosmic_api::error::MsgErr;
use cosmic_api::id::id::{Kind, Layer, Point, Port};
use cosmic_api::State;
use cosmic_api::wave::{DirectedHandler, InCtx, ReflectedCore, Router, UltraWave};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, RwLock};
use cosmic_api::sys::{Assign, Sys};

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate async_trait;

#[derive(Clone)]
pub struct DriverSkel {
    pub point: Point,
    pub router: Arc<dyn Router>,
    pub shell_tx: mpsc::Sender<DriverShellRequest>
}

impl DriverSkel {
    pub async fn ex( &self, point: Point ) -> Result<Box<dyn Core>,MsgErr> {
        let (tx,rx) = oneshot::channel();
        self.shell_tx.send(DriverShellRequest::Ex { point, tx }).await;
        Ok(rx.await??)
    }

    pub async fn assign( &self, assign: Assign) -> Result<(),MsgErr> {
        let (tx,rx) = oneshot::channel();
        self.shell_tx.send(DriverShellRequest::Assign{ assign, tx }).await;
        Ok(rx.await??)
    }
}

impl DriverSkel {
    pub fn new(point:Point, router: Arc<dyn Router>, shell_tx: mpsc::Sender<DriverShellRequest>) -> Self {
        Self {
            point,
            router,
            shell_tx
        }
    }
}

pub trait DriverFactory {
    fn kind(&self) -> Kind;
    fn create(&self, skel: DriverSkel) -> Box<dyn Driver>;
}

#[async_trait]
pub trait Driver: DirectedHandler+Send+Sync {
    fn kind(&self) -> Kind;
    async fn status(&self) -> DriverStatus;
    async fn lifecycle(&mut self, event: DriverLifecycleCall) -> Result<DriverStatus,MsgErr>;
    fn ex(&self, point: &Point, state: Option<Arc<RwLock<dyn State>>>) -> Box<dyn Core>;
    async fn assign(&self, ctx: InCtx<'_,Assign>) -> Result<Option<Arc<RwLock<dyn State>>>, MsgErr>;
}

#[derive(Clone, Eq, PartialEq, Hash)]
pub enum DriverLifecycleCall {
    Init,
    Shutdown,
}

#[derive(Clone, Eq, PartialEq, Hash)]
pub enum DriverStatus {
    Unknown,
    Started,
    Initializing,
    Ready,
    Unavailable,
}

#[derive(Clone, Eq, PartialEq, Hash)]
pub struct DriverStatusEvent {
    pub driver: Point,
    pub status: DriverStatus
}

pub trait Core: DirectedHandler+Send+Sync {
}


pub enum DriverShellRequest {
  Ex{ point: Point, tx: oneshot::Sender<Result<Box<dyn Core>,MsgErr>>},
  Assign{ assign: Assign, tx: oneshot::Sender<Result<(),MsgErr>>}
}