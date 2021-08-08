use crate::error::Error;
use crate::frame::{Frame, Reply, ReplyKind, StarMessage, ProtoFrame, StarPattern};
use crate::lane::{LaneKey, LaneWrapper, ProtoLaneEnd, LaneEnd, LaneIndex, LaneMeta, LaneCommand, LaneId, LaneSession, AbstractLaneEndpoint};
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
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

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


    pub fn add_proto_lane( &self, proto: ProtoLaneEnd, pattern: StarPattern) {
        self.tx.try_send(LaneMuxerCall::AddProtoLane{proto,pattern}).unwrap_or_default();
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
    AddProtoLane{proto:ProtoLaneEnd, pattern: StarPattern},
}

impl Call for LaneMuxerCall {}

pub struct LaneMuxer {
    rx: mpsc::Receiver<LaneMuxerCall>,
    router_tx: mpsc::Sender<RouterCall>,
    lanes: HashMap<LaneId, LaneWrapper>,
    sequence: AtomicU64
}

impl LaneMuxer {
    pub fn start(router_tx: mpsc::Sender<RouterCall>) ->  LaneMuxerApi  {
        let (tx,rx) = mpsc::channel(1024);


        tokio::spawn( async move {
            Self {
                rx,
                router_tx,
                lanes: HashMap::new(),
                sequence: AtomicU64::new(0)
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

            futures.push(self.rx.recv().boxed());

            let (call, future_index, _) = select_all(futures).await;

            let lane_id = if future_index < lanes.len() {
                lanes.get(future_index).cloned()
            }  else {
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
                       tx.send(self.lane_keys() ).unwrap_or_default();
                    }
                    LaneMuxerCall::Frame(frame)  => {

                        if lane_id.is_some()
                        {
                            let lane_id = lane_id.expect("expected a laneId");

                            match &frame {
                                Frame::Proto(proto_frame) => {

                                    if lane_id.is_proto() {


                                        match proto_frame {
                                            ProtoFrame::ReportStarKey(remote_star) => {
                                                let mut lane = self
                                                    .lanes
                                                    .remove(&lane_id)
                                                    .expect("expected lane wreapper");

                                                let mut lane = lane.expect_proto_lane();

                                                // here we have to eventually check if the remote_star matches the pattern assigned to it
                                                if lane.pattern.key_match(remote_star) {
                                                    lane.remote_star = Option::Some(remote_star.clone());
                                                    let lane: LaneMeta<LaneEnd> = lane.try_into().expect("should be able to modify into a lane since remote star is set");
                                                    let lane = LaneWrapper::Lane(lane);
                                                    self.lanes.insert(LaneId::Lane(remote_star.clone()), lane);
                                                } else {
                                                    error!("protolane attempted to claim a remote star that did not match the allowable pattern")
                                                    // we do not reinsert the lane...
                                                }
                                            }
                                            _ => {
                                                let lane = self.lanes.get(&lane_id).expect("expected a lane");
                                                let session = LaneSession::new(lane_id.clone(), lane.pattern(), lane.outgoing().out_tx.clone() );
                                                self.router_tx.try_send(RouterCall::Frame { frame,  session }).unwrap_or_default();
                                            }
                                        }
                                    }
                                }
                                _ => {
                                    let lane = self.lanes.get(&lane_id).expect("expected a lane");
                                    let session = LaneSession::new(lane_id.clone(), lane.pattern(), lane.outgoing().out_tx.clone() );
                                    self.router_tx.try_send(RouterCall::Frame { frame, session }).unwrap_or_default();
                                }
                            }
                        }
                        else {
                            error!("cannot process a frame that is not associated with a lane_id")
                        }
                    }
                    LaneMuxerCall::AddProtoLane{ proto, pattern } => {

                        self.lanes.insert(
                            LaneId::Proto(self.sequence.fetch_add(1,Ordering::Relaxed)),
                            LaneWrapper::Proto(LaneMeta::new(proto,pattern)),
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
                }
            }
        }
    }

    fn forward_frame(&mut self, lane: LaneKey, frame: Frame) {
        if let Option::Some(lane) = self.lanes.get_mut(&LaneId::Lane(lane)) {
            lane.outgoing().out_tx.try_send( LaneCommand::Frame(frame)).unwrap_or_default();
        } else {
            error!("dropped frame could not find laneKey: {}",lane.to_string() );
        }
    }

    fn lane_keys(&self) -> Vec<LaneKey> {
        let mut keys = vec!();
        for (k,_) in &self.lanes {
            if !k.is_proto() {
                keys.push(k.clone().try_into().expect("expected a lane not a protolane"));
            }
        }
        keys
    }


    fn broadcast(&mut self, frame: Frame) {
        self.broadcast_excluding(frame, &Option::None);
    }

    fn broadcast_excluding(&mut self, frame: Frame, exclude: &Option<HashSet<LaneKey>>) {
        let mut lanes = vec![];
        for lane in self.lanes.keys() {
            if let LaneId::Lane(lane) = lane {
                if exclude.is_none() || !exclude.as_ref().unwrap().contains(lane) {
                    lanes.push(lane.clone());
                }
            }
        }
        for lane in lanes {
            self.forward_frame(lane, frame.clone());
        }
    }
}



