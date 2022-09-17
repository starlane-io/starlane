use std::collections::hash_map::RandomState;
use std::collections::{HashMap, HashSet};
use std::convert::TryInto;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use cosmic_universe::id::StarKey;
use futures::future::select_all;
use futures::FutureExt;
use lru::LruCache;
use tokio::sync::{mpsc, oneshot};
use tokio::time::Duration;

use crate::error::Error;
use crate::frame::{Frame, ProtoFrame, StarMessage, StarPattern};
use crate::lane::{
    AbstractLaneEndpoint, LaneCommand, LaneEnd, LaneIndex, LaneKey, LaneMeta, LaneSession,
    LaneWrapper, OnCloseAction, ProtoLaneEnd, UltimaLaneKey,
};
use crate::message::{ProtoStarMessage, ProtoStarMessageTo};
use crate::star::core::message::CoreMessageCall;
use crate::star::shell::router::RouterCall;
use crate::star::{ForwardFrame, StarCommand, StarSkel};
use crate::util::{AsyncProcessor, AsyncRunner, Call};

#[derive(Clone)]
pub struct LaneMuxerApi {
    pub tx: mpsc::Sender<LaneMuxerCall>,
}

impl LaneMuxerApi {
    pub fn new(tx: mpsc::Sender<LaneMuxerCall>) -> Self {
        Self { tx }
    }

    pub fn forward_frame(&self, lane: LaneKey, frame: Frame) -> Result<(), Error> {
        Ok(self
            .tx
            .try_send(LaneMuxerCall::ForwardFrame { lane, frame })?)
    }

    pub fn broadcast(&self, frame: Frame, pattern: LanePattern) {
        self.tx
            .try_send(LaneMuxerCall::Broadcast { frame, pattern })
            .unwrap_or_default();
    }

    pub async fn lane_keys(&self) -> Result<Vec<UltimaLaneKey>, Error> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .try_send(LaneMuxerCall::LaneKeys(tx))
            .unwrap_or_default();

        Ok(tokio::time::timeout(Duration::from_secs(15), rx).await??)
    }

    pub fn add_proto_lane(&self, proto: ProtoLaneEnd, pattern: StarPattern) {
        self.tx
            .try_send(LaneMuxerCall::AddProtoLane { proto, pattern })
            .unwrap_or_default();
    }

    pub fn remove_lane(&self, key: LaneKey) {
        self.tx
            .try_send(LaneMuxerCall::RemoveLane(key))
            .unwrap_or_default();
    }
}

#[derive(strum_macros::Display)]
pub enum LaneMuxerCall {
    ForwardFrame {
        lane: LaneKey,
        frame: Frame,
    },
    LaneKeys(oneshot::Sender<Vec<UltimaLaneKey>>),
    Broadcast {
        frame: Frame,
        pattern: LanePattern,
    },
    Frame(Frame),
    AddProtoLane {
        proto: ProtoLaneEnd,
        pattern: StarPattern,
    },
    RemoveLane(LaneKey),
}

impl Call for LaneMuxerCall {}

pub struct LaneMuxer {
    rx: mpsc::Receiver<LaneMuxerCall>,
    router_tx: mpsc::Sender<RouterCall>,
    lanes: HashMap<LaneKey, LaneWrapper>,
    sequence: AtomicU64,
}

impl LaneMuxer {
    pub fn start(router_tx: mpsc::Sender<RouterCall>) -> LaneMuxerApi {
        let (tx, rx) = mpsc::channel(1024);

        tokio::spawn(async move {
            Self {
                rx,
                router_tx,
                lanes: HashMap::new(),
                sequence: AtomicU64::new(0),
            }
            .run()
            .await;
        });

        LaneMuxerApi { tx }
    }

    async fn run(mut self) {
        loop {
            let mut futures = vec![];
            let mut lanes = vec![];
            for (key, lane) in &mut self.lanes {
                futures.push(lane.incoming().recv().boxed());
                lanes.push(key.clone())
            }

            futures.push(self.rx.recv().boxed());

            let (call, future_index, _) = select_all(futures).await;

            let lane_key = if future_index < lanes.len() {
                lanes.get(future_index).cloned()
            } else {
                Option::None
            };

            if let Option::Some(call) = call {
                match call {
                    LaneMuxerCall::ForwardFrame { lane, frame } => {
                        self.forward_frame(lane, frame);
                    }
                    LaneMuxerCall::Broadcast { frame, pattern } => {
                        self.broadcast(frame, pattern);
                    }
                    LaneMuxerCall::LaneKeys(tx) => {
                        tx.send(self.lane_keys()).unwrap_or_default();
                    }
                    LaneMuxerCall::Frame(frame) => {
                        if lane_key.is_some() {
                            let lane_key = lane_key.expect("expected a LaneKey");
                            if let Frame::Proto(ProtoFrame::ReportStarKey(remote_star)) = &frame {
                                if lane_key.is_proto() {
                                    let mut lane =
                                        self.lanes.remove(&lane_key).expect("expected LaneWrapper");

                                    let mut lane = lane.expect_proto_lane();

                                    // here we have to eventually check if the remote_star matches the pattern assigned to it
                                    if lane.pattern.key_match(remote_star) {
                                        lane.remote_star = Option::Some(remote_star.clone());
                                        let lane: LaneMeta<LaneEnd> = lane.try_into().expect("should be able to modify into a lane since remote star is set");
                                        let lane = LaneWrapper::Lane(lane);
                                        self.lanes
                                            .insert(LaneKey::Ultima(remote_star.clone()), lane);
                                    } else {
                                        error!("protolane attempted to claim a remote star that did not match the allowable pattern");
                                        // we do not reinsert the lane... and close it
                                        lane.outgoing.out_tx.try_send(LaneCommand::Shutdown);
                                    }
                                } else {
                                    eprintln!(
                                        "received a ReportStarKey on a lane that is not proto"
                                    );
                                }
                            } else if let Frame::Close = &frame {
                                let mut lane =
                                    self.lanes.get(&lane_key).expect("expected LaneWrapper");

                                if let OnCloseAction::Remove = lane.on_close_action() {
                                    match self.lanes.remove(&lane_key) {
                                        None => {
                                            eprintln!(
                                                "Frame::Close could not find LaneKey: {}",
                                                lane_key.to_string()
                                            );
                                        }
                                        Some(_) => {}
                                    }
                                }
                            } else {
                                let lane = self.lanes.get(&lane_key).expect("expected a lane");
                                let session = LaneSession::new(
                                    lane_key.clone(),
                                    lane.pattern(),
                                    lane.outgoing().out_tx.clone(),
                                );
                                self.router_tx
                                    .try_send(RouterCall::Frame { frame, session })
                                    .unwrap_or_default();
                            }
                        } else {
                            error!("cannot process a frame that is not associated with a lane_key")
                        }
                    }
                    LaneMuxerCall::AddProtoLane { proto, pattern } => {
                        self.lanes.insert(
                            LaneKey::Proto(self.sequence.fetch_add(1, Ordering::Relaxed)),
                            LaneWrapper::Proto(LaneMeta::new(proto, pattern)),
                        );
                        /*
                        let _result =
                            lane.outgoing
                                .out_tx
                                .try_send(LaneCommand::Frame(Frame::Proto(
                                    ProtoFrame::ReportStarKey(self.skel.info.key.clone()),
                                )));

                         */
                        //                        unimplemented!("not sure how to handle adding protolanes yet");
                    }
                    LaneMuxerCall::RemoveLane(lane_key) => {
                        println!("LaneMuxer: removing lane {}", lane_key.to_string());
                        self.lanes.remove(&lane_key);
                    }
                }
            }
        }
    }

    fn forward_frame(&mut self, lane: LaneKey, frame: Frame) {
        if let Option::Some(lane) = self.lanes.get_mut(&lane) {
            lane.outgoing()
                .out_tx
                .try_send(LaneCommand::Frame(frame))
                .unwrap_or_default();
        } else {
            error!("dropped frame could not find laneKey: {}", lane.to_string());
        }
    }

    fn lane_keys(&self) -> Vec<UltimaLaneKey> {
        let mut keys = vec![];
        for (k, _) in &self.lanes {
            if !k.is_proto() {
                keys.push(
                    k.clone()
                        .try_into()
                        .expect("expected a lane not a protolane"),
                );
            }
        }
        keys
    }

    fn broadcast(&mut self, frame: Frame, pattern: LanePattern) {
        let mut lanes: Vec<LaneKey> = self.lanes.keys().map(|l| l.clone()).collect();
        lanes.retain(|lane| pattern.is_match(lane));
        for lane_key in lanes {
            self.forward_frame(lane_key, frame.clone());
        }
    }
}

pub enum LanePattern {
    None,
    Any,
    Excluding(HashSet<LaneKey>),
    Ultimas,
    Protos,
    UltimasExcluding(HashSet<UltimaLaneKey>),
    ProtosExcluding(HashSet<u64>),
}

impl LanePattern {
    pub fn is_match(&self, lane: &LaneKey) -> bool {
        match self {
            LanePattern::None => false,
            LanePattern::Any => true,
            LanePattern::Excluding(set) => !set.contains(lane),
            LanePattern::Ultimas => !lane.is_proto(),
            LanePattern::Protos => lane.is_proto(),
            LanePattern::UltimasExcluding(exclude) => match lane {
                LaneKey::Proto(_) => false,
                LaneKey::Ultima(lane) => !exclude.contains(lane),
            },
            LanePattern::ProtosExcluding(exclude) => match lane {
                LaneKey::Proto(proto) => !exclude.contains(proto),
                LaneKey::Ultima(_) => false,
            },
        }
    }
}
