use crate::error::Error;
use crate::keys::ResourceKey;
use crate::resource::{
    ResourceAddress, ResourceArchetype, ResourceCreateStrategy, ResourceKind, ResourceLocation,
    ResourceRecord, ResourceRegistration, ResourceStub,
};
use crate::star::variant::{StarVariant, StarVariantCommand, StarShellInstructions};
use crate::star::{PublicKeySource, StarKey, StarSkel, StarCommand, StarSubGraphKey};
use crate::starlane::api::{SpaceApi, StarlaneApi};
use std::convert::TryInto;
use std::str::FromStr;
use tokio::sync::oneshot;
use crate::frame::{Frame, ProtoFrame};
use crate::lane::{LaneWrapper, LaneCommand};
use std::sync::atomic::Ordering;
use tokio::time::Duration;

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
