use async_trait::async_trait;
use cosmic_api::error::MsgErr;
use cosmic_api::id::id::{Kind, Layer, Point, Port};
use cosmic_api::State;
use cosmic_api::wave::{DirectedHandler, InCtx, ReflectedCore, Router};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, RwLock};
use cosmic_api::sys::Assign;

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
    pub async fn ex( &self, point: Point ) -> Result<Box<dyn CoreEx>,MsgErr> {
        let (tx,rx) = oneshot::channel();
        self.shell_tx.send(DriverShellRequest::Ex { point, tx }).await;
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
    fn kind(&self) -> &Kind;
    fn create(&self, skel: DriverSkel) -> Box<dyn DriverCore>;
}

#[async_trait]
pub trait DriverCore: DirectedHandler+Send+Sync {
    fn kind(&self) -> &Kind;
    async fn status(&self) -> DriverStatus;
    async fn lifecycle(&self, event: DriverLifecycleCall) -> Result<DriverStatus,MsgErr>;
    fn ex(&self, point: &Point, state: Option<Arc<RwLock<dyn State>>>) -> Box<dyn CoreEx>;
    async fn assign(&self, ctx: InCtx<'_,Assign>) -> Result<ReflectedCore, MsgErr>;
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

pub trait CoreEx: DirectedHandler+Send+Sync {
    fn create(&self) -> Option<Arc<RwLock<dyn State>>>;
}


pub enum DriverShellRequest {
  Ex{ point: Point, tx: oneshot::Sender<Result<Box<dyn CoreEx>,MsgErr>>}
}