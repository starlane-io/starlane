use crate::error::Error;
use crate::frame::{Frame, Reply, ReplyKind, StarMessage, ProtoFrame};
use crate::lane::{LaneKey, LaneWrapper, ProtoLaneEndpoint, LaneEndpoint, LaneIndex, LaneMeta, LaneCommand};
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
use crate::star::shell::router::RouterCall;
use futures::FutureExt;
use std::convert::TryInto;
use futures::future::select_all;

#[derive(Clone)]
pub struct LaneMuxerApi {
    pub tx: mpsc::Sender<LaneMuxerCall>,
}

impl LaneMuxerApi {
    pub fn new(tx: mpsc::Sender<LaneMuxerCall>) -> Self {
        Self { tx }
    }

    pub fn forward_frame(&self, lane: LaneKey, frame: Frame) -> Result<(), Error> {
        Ok(self.tx.try_send(LaneMuxerCall::ForwardFrame { lane, frame })?)
    }

    pub fn broadcast_excluding(&self, frame: Frame, exclude: Option<HashSet<LaneKey>>) {
        self.tx.try_send( LaneMuxerCall::Broadcast {frame,exclude }).unwrap_or_default();
    }

    pub async fn lane_keys(&self) -> Result<Vec<LaneKey>,Error> {
      let (tx,rx) = oneshot::channel();
      self.tx
            .try_send(LaneMuxerCall::LaneKeys(tx))
            .unwrap_or_default();

      Ok(tokio::time::timeout(Duration::from_secs(15), rx).await??)
    }

    pub fn add_lane( &self, lane: LaneEndpoint) {
        self.tx.try_send(LaneMuxerCall::AddLane(lane)).unwrap_or_default();
    }

    pub fn add_proto_lane( &self, lane: ProtoLaneEndpoint) {
        self.tx.try_send(LaneMuxerCall::AddProtoLane(lane)).unwrap_or_default();
    }
}

#[derive(strum_macros::Display)]
pub enum LaneMuxerCall {
    ForwardFrame {
        lane: StarKey,
        frame: Frame,
    },
    LaneKeys(oneshot::Sender<Vec<LaneKey>>),
    Broadcast {
        frame: Frame,
        exclude: Option<HashSet<LaneKey>>,
    },
    Frame(Frame),
    AddLane(LaneEndpoint),
    AddProtoLane(ProtoLaneEndpoint)
}

impl Call for LaneMuxerCall {}

pub struct LaneMuxer {
    rx: mpsc::Receiver<LaneMuxerCall>,
    router_tx: mpsc::Sender<RouterCall>,
    lanes: HashMap<LaneKey, LaneWrapper>,
    proto_lanes: Vec<LaneWrapper>,
}

impl LaneMuxer {
    pub fn start(router_tx: mpsc::Sender<RouterCall>) ->  LaneMuxerApi  {
        let (tx,rx) = mpsc::channel(1024);


        tokio::spawn( async move {
            Self {
                rx,
                router_tx,
                lanes: HashMap::new(),
                proto_lanes: vec![]
            }.run().await;
        });

        LaneMuxerApi {
            tx
        }
    }

    async fn run(mut self)
    {
        loop {

            let mut futures = vec![];
            let mut lanes = vec![];
            for (key, lane) in &mut self.lanes {
                futures.push(lane.incoming().recv().boxed());
                lanes.push(key.clone())
            }
            let mut proto_lane_index = vec![];

            for (index, lane) in &mut self.proto_lanes.iter_mut().enumerate() {
                futures.push(lane.incoming().recv().boxed());
                proto_lane_index.push(index);
            }

            futures.push(self.rx.recv().boxed());

            let (call, future_index, _) = select_all(futures).await;

            let lane_index = if future_index < lanes.len() {
                LaneIndex::Lane(
                    lanes
                        .get(future_index)
                        .expect("expected a lane at this index")
                        .clone(),
                )
            } else if future_index < lanes.len() + proto_lane_index.len() {
                LaneIndex::ProtoLane(future_index - lanes.len())
            } else {
                LaneIndex::None
            };

            let mut lane = if future_index < lanes.len() {
                Option::Some(
                    self.lanes
                        .get_mut(lanes.get(future_index).as_ref().unwrap())
                        .expect("expected to get lane"),
                )
            } else if future_index < lanes.len() + proto_lane_index.len() {
                Option::Some(
                    self.proto_lanes
                        .get_mut(future_index - lanes.len())
                        .expect("expected to get proto_lane"),
                )
            } else {
                Option::None
            };
            if let Option::Some(call) = call {
                match call {
                    LaneMuxerCall::ForwardFrame { lane, frame } => {
                        self.forward_frame(lane, frame);
                    }
                    LaneMuxerCall::Broadcast { frame, exclude } => {
                        self.broadcast_excluding(frame,&exclude );
                    }
                    LaneMuxerCall::LaneKeys(tx) => {
                        let mut keys = vec!();
                        for (k,_) in &self.lanes {
                            keys.push(k.clone());
                        }
                        tx.send(keys);
                    }
                    LaneMuxerCall::Frame(frame)  => {
                        if lane.is_some()
                        {
                            let lane = lane.expect("expected lane to be some");
                            match &frame {
                                Frame::Proto(proto_frame) => {
                                    match proto_frame {
                                        ProtoFrame::ReportStarKey(remote_star) => {
                                            if let LaneIndex::ProtoLane(index) = lane_index {
                                                let mut lane = self
                                                    .proto_lanes
                                                    .remove(index)
                                                    .expect_proto_lane()
                                                    .unwrap();
                                                lane.remote_star = Option::Some(remote_star.clone());
                                                let lane: LaneEndpoint = lane.try_into().expect("should be able to modify into a lane");
                                                let lane = LaneWrapper::Lane(LaneMeta::new(lane));
                                                self.lanes.insert(remote_star.clone(), lane);
                                            }
                                        }
                                        _ => {
                                            self.router_tx.try_send(RouterCall::Frame { frame: frame, lane: lane.get_remote_star().expect("expected remote star") }).unwrap_or_default();
                                        }
                                    }
                                }
                                _ => {
                                    if !lane.is_proto() {
                                        self.router_tx.try_send(RouterCall::Frame { frame, lane: lane.get_remote_star().expect("expected remote star") }).unwrap_or_default();
                                    }
                                }
                            }
                        }
                    }
                    LaneMuxerCall::AddLane(lane) => {
                        self.lanes.insert(
                            lane.remote_star.clone(),
                            LaneWrapper::Lane(LaneMeta::new(lane)),
                        );
                    }
                    LaneMuxerCall::AddProtoLane(lane) => {
                        /*
                        let _result =
                            lane.outgoing
                                .out_tx
                                .try_send(LaneCommand::Frame(Frame::Proto(
                                    ProtoFrame::ReportStarKey(self.skel.info.key.clone()),
                                )));

                         */
//                        unimplemented!("not sure how to handle adding protolanes yet");
                        self.proto_lanes
                            .push(LaneWrapper::Proto(LaneMeta::new(lane)));
                    }
                }
            }
        }
    }

    fn forward_frame(&mut self, lane: LaneKey, frame: Frame) {
        if let Option::Some(lane) = self.lanes.get_mut(&lane) {
            lane.outgoing().out_tx.try_send( LaneCommand::Frame(frame)).unwrap_or_default();
        } else {
            error!("dropped frame could not find laneKey: {}",lane.to_string() );
        }
    }

    fn broadcast(&mut self, frame: Frame) {
        self.broadcast_excluding(frame, &Option::None);
    }

    fn broadcast_excluding(&mut self, frame: Frame, exclude: &Option<HashSet<StarKey>>) {
        let mut lanes = vec![];
        for lane in self.lanes.keys() {
            if exclude.is_none() || !exclude.as_ref().unwrap().contains(lane) {
                lanes.push(lane.clone());
            }
        }
        for lane in lanes {
            self.forward_frame(lane, frame.clone());
        }
    }
}


