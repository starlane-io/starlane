use tokio::sync::{mpsc, oneshot};
use crate::message::resource::ProtoMessage;
use crate::message::{ProtoStarMessage, Fail, MessageId, ProtoStarMessageTo};
use crate::util::{Call, AsyncRunner, AsyncProcessor};
use crate::star::StarSkel;
use crate::frame::{Reply, ReplyKind, StarMessage, Frame};
use tokio::time::Duration;
use crate::error::Error;
use crate::star::core::message::CoreMessageCall;

#[derive(Clone)]
pub struct RouterApi {
    pub tx: mpsc::Sender<RouterCall>
}

impl RouterApi {
    pub fn new(tx: mpsc::Sender<RouterCall> ) -> Self {
        Self {
            tx
        }
    }

    pub fn route(&self, message: StarMessage ) -> Result<(),Error> {
        Ok(self.tx.try_send(RouterCall::Route(message))?)
    }
}

pub enum RouterCall {
    Route(StarMessage)
}

impl Call for RouterCall {}

pub struct RouterComponent {
    skel: StarSkel,
}

impl RouterComponent {
    pub fn start(skel: StarSkel, rx: mpsc::Receiver<RouterCall>) {
        AsyncRunner::new(Box::new(Self { skel:skel.clone()}), skel.core_messaging_endpoint_tx.clone(), rx);
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

    fn route(&self, message: StarMessage ) {
        if message.to == self.skel.info.key {
            self.skel.core_messaging_endpoint_tx.try_send(CoreMessageCall::Message(message)).unwrap_or_default();
        } else {
            let skel = self.skel.clone();
            tokio::spawn( async move {
                if let Result::Ok(lane) = skel.star_locator_api.get_lane_for_star(message.to.clone()).await {
                    skel.lanes_api.forward(lane, Frame::StarMessage(message)).unwrap_or_default();
                } else {
                    error!("ERROR: could not get lane for star {}", message.to.to_string());
                }
            });
        }
    }

}
