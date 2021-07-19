use std::convert::TryInto;
use std::str::FromStr;
use std::sync::atomic::Ordering;

use tokio::sync::oneshot;
use tokio::time::Duration;

use crate::error::Error;
use crate::frame::{Frame, ProtoFrame};
use crate::lane::{LaneCommand, LaneWrapper};
use crate::resource::{
    ResourceAddress, ResourceArchetype, ResourceCreateStrategy, ResourceKind, ResourceLocation,
    ResourceRecord, ResourceRegistration, ResourceStub,
};
use crate::star::{PublicKeySource, StarCommand, StarKey, StarSkel, StarSubGraphKey};
use crate::star::variant::{StarShellInstructions, StarVariant, StarVariantCommand};
use crate::starlane::api::{SpaceApi, StarlaneApi};

pub struct GatewayVariant {
    skel: StarSkel,
}

impl GatewayVariant {
    pub async fn new(data: StarSkel) -> GatewayVariant {
        GatewayVariant { skel: data.clone() }
    }
}

#[async_trait]
impl StarVariant for GatewayVariant {
    fn filter(&mut self, command: &StarCommand, lane: &mut Option<&mut LaneWrapper> ) -> StarShellInstructions {

        match command {
            StarCommand::Frame(Frame::Proto(ProtoFrame::GatewaySelect)) => {

                let mut subgraph = self.skel.info.key.child_subgraph();
                subgraph.push( StarSubGraphKey::Big(self.skel.sequence.fetch_add(1,Ordering::Relaxed)) );
                let result = lane.as_mut().unwrap().outgoing().out_tx.try_send(LaneCommand::Frame(Frame::Proto(ProtoFrame::GatewayAssign(subgraph))));
                if let Result::Err(error) = result {
                    error!("lane send error: {}",error.to_string() );
                }

                StarShellInstructions::Ignore
            }
            _ => {
                StarShellInstructions::Handle
            }
        }
    }
}

impl GatewayVariant {

}
