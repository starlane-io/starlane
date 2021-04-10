use std::sync::Arc;
use std::sync::atomic::AtomicI32;

use futures::future::{err, join_all, ok, select_all};
use futures::FutureExt;
use futures::prelude::*;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{mpsc, Mutex};

use crate::constellation::Constellation;
use crate::error::Error;
use crate::id::Id;
use crate::lane::{Lane, STARLANE_PROTOCOL_VERSION, Tunnel};
use crate::message::{ProtoGram, LaneGram};
use crate::star::{Star, StarKernel, StarKey, StarShell, StarKind};
use std::cell::RefCell;

pub struct ProtoStar
{
  proto_lanes: Vec<ProtoTunnel>,
  lane_seq: AtomicI32,
  kind: StarKind,
  id: StarKey,
}

impl ProtoStar
{
    pub fn new(id: StarKey, kind: StarKind) ->Self
    {
        ProtoStar{
            lane_seq: AtomicI32::new(0),
            proto_lanes: vec![],
            kind,
            id: id
        }
    }

    pub fn add_lane( &mut self, proto_lane: ProtoTunnel)
    {
        self.proto_lanes.push(proto_lane);
    }

    pub async fn evolve(&mut self)->Result<Arc<Star>,Error>
    {
        let mut lanes = vec![];
        let mut futures = vec![];
        for proto_lane in self.proto_lanes.drain(..)
        {
            let future = proto_lane.evolve(Option::None).boxed();
            futures.push(future);
        }

        let (lane, _ready_future_index, remaining_futures) = select_all(futures).await;

        let mut lane = lane?;

        lanes.push(lane);
        for future in remaining_futures
        {
          let lane = future.await?;
          lanes.push(lane);
        }

        unimplemented!();
/*        let kernel = self.kind.evolve()?;

        Ok(Arc::new(Star{
           shell: StarShell::new( lanes, kernel )
        }))

 */
    }
}


#[derive(Clone)]
pub enum ProtoStarKernel
{
   Central,
   Mesh,
   Supervisor,
   Server,
   Gateway
}


impl ProtoStarKernel
{
    fn evolve(&self) -> Result<Box<dyn StarKernel>, Error>
    {
        Ok(Box::new(PlaceholderKernel::new()))
    }
}


pub struct PlaceholderKernel
{

}

impl PlaceholderKernel{
    pub fn new()->Self
    {
        PlaceholderKernel{}
    }
}

impl StarKernel for PlaceholderKernel
{

}


pub struct ProtoTunnel
{
    pub tx: Sender<LaneGram>,
    pub rx: Receiver<LaneGram>,
}

impl ProtoTunnel
{

    pub async fn evolve(mut self, star: Option<StarKey>) -> Result<Tunnel,Error>
    {
        self.tx.send(LaneGram::Proto(ProtoGram::StarLaneProtocolVersion(STARLANE_PROTOCOL_VERSION))).await;

        if let Option::Some(star)=star
        {
            self.tx.send(LaneGram::Proto(ProtoGram::ReportStarKey(star))).await;
        }

        // first we confirm that the version is as expected
        if let Option::Some(LaneGram::Proto(recv)) = self.rx.recv().await
        {
            match recv
            {
                ProtoGram::StarLaneProtocolVersion(version) if version == STARLANE_PROTOCOL_VERSION => {
                    // do nothing... we move onto the next step
                },
                ProtoGram::StarLaneProtocolVersion(version) => {
                    return Err(format!("wrong version: {}", version).into());
                },
                gram => {
                    return Err(format!("unexpected star gram: {} (expected to receive StarLaneProtocolVersion first)", gram).into());
                }
            }
        }
        else {
            return Err("disconnected".into());
        }

        if let Option::Some(LaneGram::Proto(recv)) = self.rx.recv().await
        {
            match recv
            {
                ProtoGram::ReportStarKey(remote_star_key) => {
                    return Ok(Tunnel{
                        remote_star: remote_star_key,
                        tx: self.tx,
                        rx: self.rx
                    });
                }
                gram => { return Err(format!("unexpected star gram: {} (expected to receive ReportStarId next)", gram).into()); }
            };
        }
        else {
            return Err("disconnected!".into())
        }
    }
}

pub fn local_lanes() ->(ProtoTunnel, ProtoTunnel)
{
    let (atx,arx) = mpsc::channel::<LaneGram>(32);
    let (btx,brx) = mpsc::channel::<LaneGram>(32);

    (ProtoTunnel {
        tx: atx,
        rx: brx
    },
     ProtoTunnel
    {
        tx: btx,
        rx: arx
    })
}
