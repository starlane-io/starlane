use std::sync::{Mutex, Weak, Arc};
use crate::lane::{Lane, ProtoLane};
use std::collections::HashMap;
use std::sync::atomic::{AtomicI32, Ordering};
use futures::future::join_all;
use crate::gram::StarGram;
use crate::error::Error;
use crate::id::Id;

pub struct ProtoStar
{
  lane_seq: AtomicI32,
  proto_lanes: Vec<ProtoLane>,
  lanes: HashMap<i32, LaneMeta>,
  kernel: Box<dyn ProtoStarKernel>
}

impl ProtoStar
{
    pub fn new( kernel: Box<dyn ProtoStarKernel>)->Result<Self,Error>
    {
        Ok(ProtoStar{
            lane_seq: AtomicI32::new(0),
            lanes: HashMap::new(),
            proto_lanes: vec![],
            kernel: kernel,
        })
    }

    pub async fn evolve(mut self)->Result<Arc<Star>,Error>
    {
        let mut futures = vec![];
        for proto_lane in self.proto_lanes.drain(..)
        {
            futures.push(proto_lane.evolve());
        }

        for result in join_all(futures ).await
        {
            let lane = result?;
            self.add_lane(lane);
        }

        // now all lanes should have exchanged version information
        // also, if this is a CENTRAL it should have broadcast it's id, prompting any listeners to request Id Sequences

        Ok(Arc::new(Star{
           shell: StarShell{
               kernel: self.kernel.evolve()?
           }
        }))
    }

    pub fn add_lane( &mut self, lane: Lane )
    {
        let id = self.lane_seq.fetch_add(1,Ordering::Relaxed);
        let wrapper = LaneMeta {
            lane: lane,
            id: id.clone()
        };

        self.lanes.insert(id.clone(),wrapper);

        if let Option::Some(star) = self.kernel.id()
        {
            self.send_to_lane(id,StarGram::ReportStarId(star));
        }
    }

    fn send_to_lane( &self, id: i32, gram: StarGram  )->Result<(),Error>
    {
        if let Some(lane) = self.lanes.get(&id)
        {
            lane.send(gram);
            Ok(())
        }
        else {
            Err(format!("cannot find lane: {}",id).into() )
        }
    }

}

pub struct Star
{
    pub shell: StarShell
}


pub struct StarShell
{
   pub kernel: Box<dyn StarKernel>
}

pub trait ProtoStarKernel: Send+Sync
{
    fn id(&self)->Option<Id>;
    fn evolve(&self) ->Result<Box<dyn StarKernel>,Error>;
}

pub trait StarKernel
{

}

pub struct LaneMeta
{
   pub id: i32,
   pub lane: Lane
}

impl LaneMeta
{
    pub async fn send( &self, gram: StarGram )->Result<(),Error>
    {
        Ok(self.lane.tx.send(gram).await?)
    }

    pub async fn receive( &mut self)->Option<StarGram>
    {
        self.lane.rx.recv().await
    }

}