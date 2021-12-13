use tokio::sync::{mpsc, oneshot};
use tokio::time::Duration;

use crate::error::Error;
use crate::frame::{Frame, ProtoFrame, StarMessage, WatchFrame};
use crate::lane::{LaneKey, LaneSession, UltimaLaneKey};
use crate::message::{ProtoStarMessage, ProtoStarMessageTo};
use crate::star::core::message::CoreMessageCall;
use crate::star::StarSkel;
use crate::star::variant::FrameVerdict;
use crate::util::{AsyncProcessor, AsyncRunner, Call};

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

    pub fn frame(&self, frame: Frame, session: LaneSession ) {
        self.tx.try_send(RouterCall::Frame{frame, session}).unwrap_or_default();
    }

}

pub enum RouterCall {
    Route(StarMessage),
    Frame{frame: Frame, session: LaneSession },
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
            RouterCall::Frame { frame, session } => {
                    self.frame( frame, session ).await;
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
                            .forward_frame(LaneKey::Ultima(lane), Frame::StarMessage(message))
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

    async fn frame(&self, frame: Frame, session: LaneSession ) {

        let verdict = match self.skel.variant_api.filter(frame,session.clone()).await
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
                Frame::SearchTraversal(traversal) => {
                    match &session.lane {
                        LaneKey::Proto(_) => {
                            error!("not expecting a search traversal from a proto lane...")
                        }
                        LaneKey::Ultima(lane) => {
                            self.skel.star_search_api.on_traversal(traversal, lane.clone() );
                        }
                    }
                }
                Frame::StarMessage(message) => {
                    self.route(message);
                }
                Frame::Watch(watch) => {
                    match watch {
                        WatchFrame::Watch(watch) => {
                            self.skel.watch_api.watch(watch, session );
                        }
                        WatchFrame::UnWatch(key) => {
                            self.skel.watch_api.un_watch(key);
                        }
                        WatchFrame::Notify(notification) => {
                            self.skel.watch_api.notify(notification);
                        }
                    }
                }
                Frame::Close => {}
            }
        }
    }
}
