use std::sync::atomic::Ordering;

use crate::frame::{Frame, ProtoFrame};
use crate::lane::{LaneCommand, LaneWrapper, LaneKey};

use crate::star::variant::{FrameVerdict, VariantCall};
use crate::star::{StarCommand, StarKey, StarSkel, StarSubGraphKey};
use crate::util::{AsyncProcessor, AsyncRunner};
use tokio::sync::mpsc;

pub struct GatewayVariant {
    skel: StarSkel,
}

impl GatewayVariant{
    pub fn start(skel: StarSkel, rx: mpsc::Receiver<VariantCall>) {
        AsyncRunner::new(
            Box::new(Self { skel: skel.clone() }),
            skel.variant_api.tx.clone(),
            rx,
        );
    }
}

#[async_trait]
impl AsyncProcessor<VariantCall> for GatewayVariant{
    async fn process(&mut self, call: VariantCall) {
        match call {
            VariantCall::Init(tx) => {
                tx.send(Ok(()));
            }
            VariantCall::Frame { frame, lane, tx } => {
                tx.send(self.filter(frame, lane));
            }
        }
    }
}


impl GatewayVariant {
    fn filter(
        &mut self,
        frame: Frame,
        lane: LaneKey,
    ) -> FrameVerdict {
        match frame{
            Frame::Proto(ProtoFrame::GatewaySelect) => {
                let mut subgraph = self.skel.info.key.child_subgraph();
                subgraph.push(StarSubGraphKey::Big(
                    self.skel.sequence.fetch_add(1, Ordering::Relaxed),
                ));

                let result = self.skel.lane_muxer_api.forward_frame(lane,
                Frame::Proto(ProtoFrame::GatewayAssign(
                    subgraph,
                )));

               if let Result::Err(error) = result {
                    error!("lane send error: {}", error.to_string());
                }

                FrameVerdict::Ignore
            }
            _ => FrameVerdict::Handle(frame),
        }
    }
}

