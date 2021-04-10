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
use crate::lane::{Lane, STARLANE_PROTOCOL_VERSION};
use crate::message::ProtoGram;
use crate::star::{Star, StarKernel, StarKey, StarShell, StarKind};
use std::cell::RefCell;

pub struct ProtoConstellation
{
    pub proto_stars: Vec<Arc<Mutex<ProtoStar>>>
}

impl ProtoConstellation
{

    /*
    pub fn new_standalone()->Self
    {
        let mut protos = vec!();
        let mut central = ProtoStar::new(StarKey::new(Id::new(0, 0)), Box::new(ProtoCentralKernel::new() ));
        let mut mesh = ProtoStar::new(StarKey::new(vec![], Id::new(0, 1)), Box::new(ProtoMeshKernel::new() ));


        let (mut lane1,mut lane2) = local_lane();
        central.add_lane(lane1);
        mesh.add_lane(lane2);

        protos.push( central );
        protos.push( mesh );

        ProtoConstellation{
            proto_stars: protos
        }
    }
     */

    pub fn new()->Self
    {
        ProtoConstellation{
            proto_stars: vec![]
        }
    }

    pub async fn evolve(&mut self)->Result<Constellation,Error>
    {
        let mut futures = vec![];
        for mut proto_star in self.proto_stars.drain(..)
        {

            let mut proto_star = proto_star.lock().await;
            let future = proto_star.evolve();
            futures.push(future);
        }
        let mut stars = vec![];
        for result in join_all(futures).await
        {
            let star = result?;
            stars.push(star);
        }

        Ok(Constellation{
            stars: stars
        })
    }
}

pub struct ProtoStar
{
  proto_lanes: Vec<ProtoLane>,
  lane_seq: AtomicI32,
  kind: StarKind,
  id: StarKey
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

    pub fn add_lane( &mut self, proto_lane: ProtoLane )
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


pub struct ProtoLane
{
    pub tx: Sender<ProtoGram>,
    pub rx: Receiver<ProtoGram>,
}

impl ProtoLane
{

    pub async fn evolve(mut self, star: Option<Id>) -> Result<Lane,Error>
    {
        self.tx.send(ProtoGram::StarLaneProtocolVersion(STARLANE_PROTOCOL_VERSION)).await;

        if let Option::Some(star)=star
        {
            self.tx.send(ProtoGram::ReportStarId(star)).await;
        }

        // first we confirm that the version is as expected
        let recv = self.rx.recv().await;

        match recv
        {
            Some(ProtoGram::StarLaneProtocolVersion(version)) if version == STARLANE_PROTOCOL_VERSION => {
                // do nothing... we move onto the next step
            },
            Some(ProtoGram::StarLaneProtocolVersion(version)) => {
                return Err(format!("wrong version: {}",version).into());},
            Some(gram) => {
                return Err(format!("unexpected star gram: {} (expected to receive StarLaneProtocolVersion first)",gram).into());}
            None => {
                return Err("disconnected".into());},
        }

        match self.rx.recv().await
        {
            Some(ProtoGram::ReportStarId(remote_star_id))=>{
                return Ok( Lane {
                    remote_star: remote_star_id,
                    tx: self.tx,
                    rx: self.rx
                });
            },
            Some(gram) => {return Err(format!("unexpected star gram: {} (expected to receive ReportStarId next)",gram).into());}
            None => {return Err("disconnected".into());},
        };


    }
}

pub fn local_lanes() ->(ProtoLane, ProtoLane)
{
    let (atx,arx) = mpsc::channel::<ProtoGram>(32);
    let (btx,brx) = mpsc::channel::<ProtoGram>(32);

    (ProtoLane{
        tx: atx,
        rx: brx
    },
    ProtoLane
    {
        tx: btx,
        rx: arx
    })
}
