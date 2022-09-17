use std::collections::hash_map::RandomState;
use std::collections::{HashMap, HashSet};

use cosmic_universe::loc::StarKey;
use lru::LruCache;
use tokio::sync::{mpsc, oneshot};
use tokio::time::Duration;

use crate::error::Error;
use crate::frame::{Frame, StarMessage, StarPattern};
use crate::lane::{LaneWrapper, UltimaLaneKey};
use crate::message::{ProtoStarMessage, ProtoStarMessageTo};
use crate::star::core::message::CoreMessageCall;
use crate::star::{ForwardFrame, StarCommand, StarSkel};
use crate::util::{AsyncProcessor, AsyncRunner, Call};

#[derive(Clone)]
pub struct GoldenPathApi {
    pub tx: mpsc::Sender<GoldenCall>,
}

impl GoldenPathApi {
    pub fn new(tx: mpsc::Sender<GoldenCall>) -> Self {
        Self { tx }
    }
    pub async fn golden_lane_leading_to_star(&self, star: StarKey) -> Result<UltimaLaneKey, Error> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .try_send(GoldenCall::GoldenLaneLeadingToStar {
                star,
                tx,
                try_search: true,
            })
            .unwrap_or_default();
        Ok(tokio::time::timeout(Duration::from_secs(15), rx).await???)
    }

    fn insert_hops(&self, hops: HashMap<UltimaLaneKey, HashMap<StarKey, usize>>) {
        self.tx
            .try_send(GoldenCall::InsertHops(hops))
            .unwrap_or_default();
    }
}

pub enum GoldenCall {
    GoldenLaneLeadingToStar {
        star: StarKey,
        tx: oneshot::Sender<Result<UltimaLaneKey, Error>>,
        try_search: bool,
    },
    InsertHops(HashMap<UltimaLaneKey, HashMap<StarKey, usize>>),
}

impl Call for GoldenCall {}

pub struct GoldenPathComponent {
    skel: StarSkel,
    star_to_lane: LruCache<StarKey, HashMap<UltimaLaneKey, usize>>,
}

impl GoldenPathComponent {
    pub fn start(skel: StarSkel, rx: mpsc::Receiver<GoldenCall>) {
        AsyncRunner::new(
            Box::new(Self {
                skel: skel.clone(),
                star_to_lane: LruCache::new(256),
            }),
            skel.golden_path_api.tx.clone(),
            rx,
        );
    }
}

#[async_trait]
impl AsyncProcessor<GoldenCall> for GoldenPathComponent {
    async fn process(&mut self, call: GoldenCall) {
        match call {
            GoldenCall::GoldenLaneLeadingToStar {
                star,
                tx,
                try_search,
            } => {
                self.golden_path_leading_to_star(star, tx, try_search);
            }
            GoldenCall::InsertHops(hops) => {
                for (lane, star_hops) in hops {
                    for (star, hops) in star_hops {
                        let mut lane_to_hops = match self.star_to_lane.get(&star) {
                            None => HashMap::new(),
                            Some(map) => map.clone(),
                        };
                        lane_to_hops.insert(lane.clone(), hops);
                        self.star_to_lane.put(star, lane_to_hops);
                    }
                }
            }
        }
    }
}

impl GoldenPathComponent {
    fn golden_path_leading_to_star(
        &mut self,
        star: StarKey,
        tx: oneshot::Sender<Result<UltimaLaneKey, Error>>,
        try_search: bool,
    ) {
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
        } else if try_search {
            tokio::spawn(async move {
                let result = skel
                    .star_search_api
                    .search(StarPattern::StarKey(star.clone()))
                    .await;

                match result {
                    Ok(hits) => {
                        skel.golden_path_api.insert_hops(hits.lane_hits);
                        skel.golden_path_api
                            .tx
                            .try_send(GoldenCall::GoldenLaneLeadingToStar {
                                star,
                                tx,
                                try_search: false,
                            })
                            .unwrap_or_default();
                    }
                    Err(error) => {
                        tx.send(Err(error));
                    }
                }
            });
        } else {
            tx.send(Err(
                format!("could not find star: {}", star.to_string()).into()
            ));
        }
    }
}
