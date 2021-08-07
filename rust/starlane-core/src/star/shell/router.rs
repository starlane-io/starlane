use crate::error::Error;
use crate::frame::{Frame, Reply, ReplyKind, StarMessage, ProtoFrame};
use crate::message::resource::ProtoMessage;
use crate::message::{Fail, MessageId, ProtoStarMessage, ProtoStarMessageTo};
use crate::star::core::message::CoreMessageCall;
use crate::star::StarSkel;
use crate::util::{AsyncProcessor, AsyncRunner, Call};
use tokio::sync::{mpsc, oneshot};
use tokio::time::Duration;
use crate::lane::LaneKey;
use crate::star::variant::FrameVerdict;

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

    pub fn frame(&self, frame: Frame, lane: LaneKey) {
        self.tx.try_send(RouterCall::Frame{frame,lane}).unwrap_or_default();
    }
}

pub enum RouterCall {
    Route(StarMessage),
    Frame{frame: Frame, lane: LaneKey},
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
            RouterCall::Frame { frame, lane } => {
                    self.frame( frame, lane ).await;
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
                        skel.lane_muxer_api
                            .forward_frame(lane, Frame::StarMessage(message))
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

    async fn frame(&self, frame: Frame, lane: LaneKey ) {

        let verdict = match self.skel.variant_api.filter(frame,lane.clone()).await
                      {
                          Ok(verdict) => verdict,
                          Err(err) => {
                              error!("FrameVerdict ERROR: {}", err.to_string() );
                              FrameVerdict::Ignore
                          }
                      };

        if let FrameVerdict::Handle(frame) = verdict
        {
            match frame {
                Frame::Proto(_) => {}
                Frame::Diagnose(_) => {}
                Frame::SearchTraversal(traverasal) => {
                    self.skel.star_search_api.on_traversal(traverasal, lane);
                }
                Frame::StarMessage(message) => {
                    self.route(message);
                }
                Frame::Ping => {}
                Frame::Pong => {}
                Frame::Close => {}
            }
        }
    }
}
