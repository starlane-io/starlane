use crate::error::Error;
use crate::frame::{Frame, Reply, ReplyKind, StarMessage};
use crate::lane::LaneKey;
use crate::message::resource::ProtoMessage;
use crate::message::{Fail, MessageId, ProtoStarMessage, ProtoStarMessageTo};
use crate::star::core::message::CoreMessageCall;
use crate::star::{ForwardFrame, StarCommand, StarKey, StarSkel};
use crate::util::{AsyncProcessor, AsyncRunner, Call};
use tokio::sync::{mpsc, oneshot};
use tokio::time::Duration;

#[derive(Clone)]
pub struct LanesApi {
    pub tx: mpsc::Sender<LanesCall>,
}

impl LanesApi {
    pub fn new(tx: mpsc::Sender<LanesCall>) -> Self {
        Self { tx }
    }

    pub fn forward(&self, lane: LaneKey, frame: Frame) -> Result<(), Error> {
        Ok(self.tx.try_send(LanesCall::Frame { lane, frame })?)
    }
}

pub enum LanesCall {
    Frame { lane: StarKey, frame: Frame },
}

impl Call for LanesCall {}

pub struct LanesComponent {
    skel: StarSkel,
}

impl LanesComponent {
    pub fn start(skel: StarSkel, rx: mpsc::Receiver<LanesCall>) {
        AsyncRunner::new(
            Box::new(Self { skel: skel.clone() }),
            skel.lanes_api.tx.clone(),
            rx,
        );
    }
}

#[async_trait]
impl AsyncProcessor<LanesCall> for LanesComponent {
    async fn process(&mut self, call: LanesCall) {
        match call {
            LanesCall::Frame { lane, frame } => {
                self.frame(lane, frame);
            }
        }
    }
}

impl LanesComponent {
    fn frame(&self, lane: StarKey, frame: Frame) {
        self.skel
            .star_tx
            .try_send(StarCommand::ForwardFrame(ForwardFrame { to: lane, frame }))
            .unwrap_or_default();
    }
}
