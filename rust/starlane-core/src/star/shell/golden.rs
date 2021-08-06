use crate::error::Error;
use crate::frame::{Frame, Reply, ReplyKind, StarMessage, StarPattern};
use crate::lane::{LaneKey, LaneWrapper};
use crate::message::resource::ProtoMessage;
use crate::message::{Fail, MessageId, ProtoStarMessage, ProtoStarMessageTo};
use crate::star::core::message::CoreMessageCall;
use crate::star::{ForwardFrame, StarCommand, StarKey, StarSkel};
use crate::util::{AsyncProcessor, AsyncRunner, Call};
use std::collections::{HashSet, HashMap};
use tokio::sync::{mpsc, oneshot};
use tokio::time::Duration;
use lru::LruCache;
use std::collections::hash_map::RandomState;

#[derive(Clone)]
pub struct GoldenPathApi {
    pub tx: mpsc::Sender<GoldenCall>,
}

impl GoldenPathApi {
    pub fn new(tx: mpsc::Sender<GoldenCall>) -> Self {
        Self { tx }
    }
    pub async fn golden_lane_leading_to_star(
        &self,
        star: StarKey,
    ) -> Result<LaneKey, Error> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .try_send(GoldenCall::GoldenLaneLeadingToStar { star, tx })
            .unwrap_or_default();
         Ok(tokio::time::timeout(Duration::from_secs(15), rx).await???)
    }

    fn insert_hops( &self, hops: HashMap<StarKey,HashMap<LaneKey,usize>>) {
        self.tx.try_send( GoldenCall::InsertHops(hops)).unwrap_or_default();
    }
}

pub enum GoldenCall {
    GoldenLaneLeadingToStar {
        star: StarKey,
        tx: oneshot::Sender<Result<LaneKey,Error>>,
    },
    InsertHops(HashMap<StarKey,HashMap<LaneKey,usize>>)
}

impl Call for GoldenCall {}

pub struct GoldenPathComponent {
    skel: StarSkel,
    star_to_lane: LruCache<StarKey,HashMap<LaneKey,usize>>
}

impl GoldenPathComponent {
    pub fn start(skel: StarSkel, rx: mpsc::Receiver<GoldenCall>) {
        AsyncRunner::new(
            Box::new(Self { skel: skel.clone(), star_to_lane: LruCache::new(256 ) }),
            skel.golden_path_api.tx.clone(),
            rx,
        );
    }
}

#[async_trait]
impl AsyncProcessor<GoldenCall> for GoldenPathComponent {
    async fn process(&mut self, call: GoldenCall) {
        match call {
            GoldenCall::GoldenLaneLeadingToStar { star, tx } => {
                self.golden_path_leading_to_star(star,tx)
            }
            GoldenCall::InsertHops(hops) => {
                for (star,lane_hops) in hops {
                    self.star_to_lane.put( star, lane_hops );
                }
            }
        }
    }
}

impl GoldenPathComponent {

    fn golden_path_leading_to_star(&mut self, star: StarKey, tx: oneshot::Sender<Result<LaneKey,Error>>)   {
        let skel = self.skel.clone();

        if let Option::Some(lanes) = self.star_to_lane.get(&star) {
            if lanes.is_empty() {
                tx.send(Err("lanes are empty".into())).unwrap_or_default();
                return;
            }

            let min_hops = usize::MAX;
            let mut rtn = Option::None;

            for (lane, hops) in lanes {
                if *hops < min_hops {
                    rtn = Option::Some(lane.clone());
                }
            }

            tx.send(Ok(rtn.unwrap())).unwrap_or_default();
        } else {
            tokio::spawn( async move {
                let result = skel.star_search_api.search(StarPattern::StarKey(star.clone())).await;

                match result {
                    Ok(hops) => {
                        skel.golden_path_api.insert_hops(hops.lane_hits);
                        skel.golden_path_api.tx.try_send( GoldenCall::GoldenLaneLeadingToStar {star,tx}).unwrap_or_default();
                    }
                    Err(error) => {
                        tx.send(Err(error));
                    }
                }

            });
        }
    }

}

