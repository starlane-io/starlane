use tokio::sync::{mpsc, oneshot};
use crate::message::resource::ProtoMessage;
use crate::message::{ProtoStarMessage, Fail, MessageId, ProtoStarMessageTo};
use crate::util::{Call, AsyncRunner, AsyncProcessor};
use crate::star::{StarSkel, StarKey, StarCommand, ForwardFrame};
use crate::frame::{Reply, ReplyKind, StarMessage, Frame};
use tokio::time::Duration;
use crate::error::Error;
use crate::star::core::message::CoreMessageCall;
use crate::lane::LaneKey;

#[derive(Clone)]
pub struct LanesApi {
    pub tx: mpsc::Sender<LanesCall>
}

impl LanesApi {
    pub fn new(tx: mpsc::Sender<LanesCall> ) -> Self {
        Self {
            tx
        }
    }

    pub fn forward(&self, lane: LaneKey, frame: Frame ) -> Result<(),Error> {
        Ok(self.tx.try_send(LanesCall::Frame{lane, frame})?)
    }
}

pub enum LanesCall {
    Frame{lane:StarKey, frame:Frame}
}

impl Call for LanesCall {}

pub struct LanesComponent {
    skel: StarSkel,
}

impl LanesComponent {
    pub fn start(skel: StarSkel, rx: mpsc::Receiver<LanesCall>) {
        AsyncRunner::new(Box::new(Self { skel:skel.clone()}), skel.lanes_api.tx.clone(), rx);
    }
}

#[async_trait]
impl AsyncProcessor<LanesCall> for LanesComponent {
    async fn process(&mut self, call: LanesCall) {
        match call {
            LanesCall::Frame { lane, frame } => {
                self.frame(lane,frame);
            }
        }
    }
}

impl LanesComponent {

    fn frame( &self, lane: StarKey, frame: Frame  ) {
        self.skel.star_tx.try_send(StarCommand::ForwardFrame(ForwardFrame{
            to: lane,
            frame
        })).unwrap_or_default();
    }

}
