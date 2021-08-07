use std::fmt;

use tokio::sync::oneshot;

use crate::error::Error;
use crate::frame::{StarMessage, Frame};
use crate::lane::{LaneWrapper, LaneKey};
use crate::star::variant::central::CentralVariant;
use crate::star::variant::gateway::GatewayVariant;
use crate::star::variant::web::WebVariant;
use crate::star::{StarCommand, StarKind, StarSkel};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::Duration;
use crate::util::{AsyncRunner, AsyncProcessor, Call};

pub mod central;
pub mod gateway;
pub mod web;


#[derive(Clone)]
pub struct VariantApi  {
    pub tx: mpsc::Sender<VariantCall>
}

impl VariantApi {
    pub fn new( tx: mpsc::Sender<VariantCall>) -> Self {
        Self {
            tx
        }
    }
    pub async fn init(&self) -> Result<(),Error> {
        let (tx,rx) = oneshot::channel();
        self.tx.try_send(VariantCall::Init(tx)).unwrap_or_default();
        rx.await?
    }

    pub async fn filter(&self, frame: Frame, lane: LaneKey) -> Result<FrameVerdict,Error> {
        let (tx,rx) = oneshot::channel();
        let call = VariantCall::Frame {frame,lane,tx};
        self.tx.try_send(call).unwrap_or_default();
        Ok(tokio::time::timeout( Duration::from_secs(15), rx).await??)
    }
}

#[derive(strum_macros::Display)]
pub enum VariantCall {
    Init(oneshot::Sender<Result<(),Error>>),
    Frame{ frame: Frame, lane: LaneKey, tx: oneshot::Sender<FrameVerdict>}
}

impl Call for VariantCall {}

pub enum FrameVerdict {
    Ignore,
    Handle(Frame),
}

pub struct NoVariant{
    skel: StarSkel,
}

impl NoVariant{
    pub fn start(skel: StarSkel, rx: mpsc::Receiver<VariantCall>) {
        AsyncRunner::new(
            Box::new(Self { skel: skel.clone() }),
            skel.variant_api.tx.clone(),
            rx,
        );
    }
}

#[async_trait]
impl AsyncProcessor<VariantCall> for NoVariant{
    async fn process(&mut self, call: VariantCall) {
        match call {
            VariantCall::Init(tx) => {
                tx.send(Ok(()));
            }
            VariantCall::Frame { frame, lane, tx } => {
                tx.send(FrameVerdict::Handle(frame));
            }
        }
    }
}



pub fn start_variant( skel: StarSkel, rx: mpsc::Receiver<VariantCall>) {
    let kind = skel.info.kind.clone();
    match kind {
        StarKind::Central => CentralVariant::start(skel, rx),
        StarKind::Gateway => GatewayVariant::start(skel, rx),
        StarKind::Web => WebVariant::start(skel, rx),
        _ => NoVariant::start(skel, rx)
    }
}

