use crate::error::Error;
use crate::frame::{Frame, Reply, ReplyKind, StarMessage};
use crate::message::resource::ProtoMessage;
use crate::message::{Fail, MessageId, ProtoStarMessage, ProtoStarMessageTo};
use crate::star::core::message::CoreMessageCall;
use crate::star::StarSkel;
use crate::util::{AsyncProcessor, AsyncRunner, Call};
use tokio::sync::{mpsc, oneshot};
use tokio::time::Duration;

#[derive(Clone)]
pub struct RouterApi {
    pub tx: mpsc::Sender<RouterCall>,
}

impl RouterApi {
    pub fn new(tx: mpsc::Sender<RouterCall>) -> Self {
        Self { tx }
    }

    pub fn route(&self, message: StarMessage) -> Result<(), Error> {
        Ok(self.tx.try_send(RouterCall::Route(message))?)
    }
}

pub enum RouterCall {
    Route(StarMessage),
}

impl Call for RouterCall {}

pub struct RouterComponent {
    skel: StarSkel,
}

impl RouterComponent {
    pub fn start(skel: StarSkel, rx: mpsc::Receiver<RouterCall>) {
        AsyncRunner::new(
            Box::new(Self { skel: skel.clone() }),
            skel.router_api.tx.clone(),
            rx,
        );
    }
}

#[async_trait]
impl AsyncProcessor<RouterCall> for RouterComponent {
    async fn process(&mut self, call: RouterCall) {
        match call {
            RouterCall::Route(message) => {
                self.route(message);
            }
        }
    }
}

impl RouterComponent {
    fn route(&self, message: StarMessage) {
        let skel = self.skel.clone();
        tokio::spawn(async move {
            if message.to == skel.info.key {
                if message.reply_to.is_some() {
                    skel.messaging_api.on_reply(message);
                } else {
                    skel.core_messaging_endpoint_tx
                        .try_send(CoreMessageCall::Message(message))
                        .unwrap_or_default();
                }
            } else {
                if let Result::Ok(lane) = skel
                    .golden_path_api.golden_lane_leading_to_star(message.to.clone())
                    .await
                {
                    skel.lanes_api
                        .forward(lane, Frame::StarMessage(message))
                        .unwrap_or_default();
                } else {
                    error!(
                        "ERROR: could not get lane for star {}",
                        message.to.to_string()
                    );
                }
            }
        });
    }
}
