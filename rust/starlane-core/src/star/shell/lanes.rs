use crate::error::Error;
use crate::frame::{Frame, Reply, ReplyKind, StarMessage};
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
pub struct LanesApi {
    pub tx: mpsc::Sender<LanesCall>,
}

impl LanesApi {
    pub fn new(tx: mpsc::Sender<LanesCall>) -> Self {
        Self { tx }
    }

    pub fn forward(&self, lane: LaneKey, frame: Frame) -> Result<(), Error> {
        Ok(self.tx.try_send(LanesCall::Frame { lane, frame })?)
    }

    pub fn broadcast_excluding(&self, frame: Frame, exclude: Option<HashSet<LaneKey>>) {
        self.tx.try_send( LanesCall::Broadcast {frame,exclude }).unwrap_or_default();
    }

    pub async fn lane_keys(&self) -> Result<Vec<LaneKey>,Error> {
      let (tx,rx) = oneshot::channel();
      self.tx
            .try_send(LanesCall::LaneKeys(tx))
            .unwrap_or_default();

      Ok(tokio::time::timeout(Duration::from_secs(15), rx).await??)
    }
}

pub enum LanesCall {
    Frame {
        lane: StarKey,
        frame: Frame,
    },
    LaneKeys(oneshot::Sender<Vec<LaneKey>>),
    Broadcast {
        frame: Frame,
        exclude: Option<HashSet<LaneKey>>,
    },
}

impl Call for LanesCall {}

pub struct LanesComponent {
    skel: StarSkel,
    star_to_lane: LruCache<StarKey,HashMap<LaneKey,usize>>
}

impl LanesComponent {
    pub fn start(skel: StarSkel, rx: mpsc::Receiver<LanesCall>) {
        AsyncRunner::new(
            Box::new(Self { skel: skel.clone(), star_to_lane: LruCache::new(256 ) }),
            skel.lanes_api.tx.clone(),
            rx,
        );
    }
}

#[async_trait]
impl AsyncProcessor<LanesCall> for LanesComponent {
    async fn process(&mut self, call: LanesCall) {
        match call {
            LanesCall::Frame { lane, frame } => {
                self.frame(lane, frame);
            }
            LanesCall::Broadcast { frame, exclude } => {
                //self.broadcast_excluding(frame, exclude);
                self.skel.star_tx.try_send( StarCommand::Broadcast { frame, exclude } ).unwrap_or_default();
            }
            LanesCall::LaneKeys(tx) => {
                self.skel.star_tx.try_send( StarCommand::LaneKeys(tx) ).unwrap_or_default();
            }
        }
    }
}

impl LanesComponent {
    fn frame(&self, lane: LaneKey, frame: Frame) {
        self.skel
            .star_tx
            .try_send(StarCommand::ForwardFrame(ForwardFrame { to: lane, frame }))
            .unwrap_or_default();
    }

    /*
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
            self.frame(lane, frame.clone()).await;
        }
    }


     */
}

