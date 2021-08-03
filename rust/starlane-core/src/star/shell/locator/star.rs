use tokio::sync::{mpsc, oneshot};
use crate::message::resource::ProtoMessage;
use crate::message::{ProtoStarMessage, Fail, MessageId, ProtoStarMessageTo};
use crate::util::{Call, AsyncRunner, AsyncProcessor};
use crate::star::{StarSkel, StarKey};
use crate::frame::{Reply, ReplyKind, StarMessage};
use tokio::time::Duration;
use crate::error::Error;
use crate::star::core::message::CoreMessageCall;
use crate::lane::LaneKey;

#[derive(Clone)]
pub struct StarLocatorApi {
    pub tx: mpsc::Sender<StarLocateCall>
}

impl StarLocatorApi {
    pub fn new(tx: mpsc::Sender<StarLocateCall> ) -> Self {
        Self {
            tx
        }
    }

    pub async fn get_lane_for_star(&self, star: StarKey ) -> Result<LaneKey,Error> {
        let( tx, rx ) = oneshot::channel();
        self.tx.try_send(StarLocateCall::GetLaneForStar {star,tx})?;
        Ok(tokio::time::timeout(Duration::from_secs(15), rx).await???)
    }
}

pub enum StarLocateCall {
    GetLaneForStar {star: StarKey, tx: oneshot::Sender<Result<LaneKey,Error>>}
}

impl Call for StarLocateCall {}

pub struct StarLocatorComponent {
    skel: StarSkel,
}

impl StarLocatorComponent {
    pub fn start(skel: StarSkel, rx: mpsc::Receiver<StarLocateCall>) {
        AsyncRunner::new(Box::new(Self { skel:skel.clone()}), skel.star_locator_api.tx.clone(), rx);
    }
}

#[async_trait]
impl AsyncProcessor<StarLocateCall> for StarLocatorComponent {
    async fn process(&mut self, call: StarLocateCall) {
        match call {
            StarLocateCall::GetLaneForStar{star,tx} => {
                self.get_lane_for_star(star,tx);
            }
        }
    }
}

impl StarLocatorComponent {

    fn get_lane_for_star( &self, star: StarKey, tx: oneshot::Sender<Result<LaneKey,Error>> ) {

    }

}
