use std::fmt;

use tokio::sync::oneshot;

use crate::error::Error;
use crate::frame::{Frame, StarMessage};
use crate::lane::{LaneSession, LaneWrapper, UltimaLaneKey};
use crate::star::variant::central::CentralVariant;
use crate::star::variant::web::WebVariant;
use crate::star::{StarCommand, StarKind, StarSkel};
use crate::util::{AsyncProcessor, AsyncRunner, Call};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::Duration;

pub mod central;
pub mod web;

#[derive(Clone)]
pub struct VariantApi {
    pub tx: mpsc::Sender<VariantCall>,
}

impl VariantApi {
    pub fn new(tx: mpsc::Sender<VariantCall>) -> Self {
        Self { tx }
    }
    pub async fn init(&self) -> Result<(), Error> {
        let (tx, rx) = oneshot::channel();
        self.tx.try_send(VariantCall::Init(tx)).unwrap_or_default();
        rx.await?
    }

    pub async fn filter(&self, frame: Frame, session: LaneSession) -> Result<FrameVerdict, Error> {
        let (tx, rx) = oneshot::channel();
        let call = VariantCall::Frame { frame, session, tx };
        self.tx.try_send(call).unwrap_or_default();
        Ok(tokio::time::timeout(Duration::from_secs(15), rx).await??)
    }
}

#[derive(strum_macros::Display)]
pub enum VariantCall {
    Init(oneshot::Sender<Result<(), Error>>),
    Frame {
        frame: Frame,
        session: LaneSession,
        tx: oneshot::Sender<FrameVerdict>,
    },
}

impl Call for VariantCall {}

pub enum FrameVerdict {
    Ignore,
    Handle(Frame),
}

pub struct NoVariant {
    skel: StarSkel,
}

impl NoVariant {
    pub fn start(skel: StarSkel, rx: mpsc::Receiver<VariantCall>) {
        AsyncRunner::new(
            Box::new(Self { skel: skel.clone() }),
            skel.variant_api.tx.clone(),
            rx,
        );
    }
}

#[async_trait]
impl AsyncProcessor<VariantCall> for NoVariant {
    async fn process(&mut self, call: VariantCall) {
        match call {
            VariantCall::Init(tx) => {
                tx.send(Ok(())).unwrap_or_default();
            }
            VariantCall::Frame { frame, session, tx } => {
                tx.send(FrameVerdict::Handle(frame)).unwrap_or_default();
            }
        }
    }
}

pub fn start_variant(skel: StarSkel, rx: mpsc::Receiver<VariantCall>) {
    let kind = skel.info.kind.clone();
    match kind {
        StarKind::Central => CentralVariant::start(skel, rx),
        StarKind::Web => WebVariant::start(skel, rx),
        _ => NoVariant::start(skel, rx),
    }
}
