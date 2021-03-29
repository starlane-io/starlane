use std::sync::{Mutex, Weak, Arc};
use crate::lane::{Lane, ProtoLane};
use std::collections::HashMap;
use std::sync::atomic::{AtomicI32, Ordering};
use futures::future::join_all;
use futures::future::select_all;
use crate::gram::StarGram;
use crate::error::Error;
use crate::id::{Id, IdSeq};
use futures::FutureExt;

pub struct ProtoStar
{
  lane_seq: AtomicI32,
  proto_lanes: Vec<ProtoLane>,
  kernel: Box<dyn ProtoStarKernel>,
  id: Option<Id>
}

impl ProtoStar
{
    pub fn new( kernel: Box<dyn ProtoStarKernel>)->Result<Self,Error>
    {
        Ok(ProtoStar{
            lane_seq: AtomicI32::new(0),
            proto_lanes: vec![],
            kernel: kernel,
            id: Option::None
        })
    }

    pub async fn evolve(mut self)->Result<Arc<Star>,Error>
    {
        self.id = self.kernel.default_id();
        let mut lanes = vec![];
        let mut futures = vec![];
        for proto_lane in self.proto_lanes.drain(..)
        {
            let future = proto_lane.evolve(self.kernel.default_id() ).boxed();
            futures.push(future);
        }

        let (lane, _ready_future_index, remaining_futures) = select_all(futures).await;

        let mut lane = lane?;

        if self.id.is_some()
        {
            lanes.push(lane);
            for future in remaining_futures
            {
                let lane = future.await?;
                lanes.push(lane);
            }
        }
        else
        {
            lane.tx.send(StarGram::RequestUniqueSequence);
            let sequence = match lane.rx.recv().await
            {
                None => {
                    return Err("disconnection".into());
                }
                Some(gram) => {
                    match gram
                    {
                        StarGram::AssignUniqueSequence(sequence) => {sequence}
                        _ => {
                            return Err(format!("unexpected gram in assign id phase: {}",gram).into());
                        }
                    }
                }
            };
            let sequence = IdSeq::new(sequence);
            self.id = Option::Some(sequence.next());
            for future in remaining_futures
            {
                let lane = future.await?;
                lane.tx.send( StarGram::ReportStarId(self.id.unwrap()));
                lanes.push(lane);
            }
        }

        let kernel = self.kernel.evolve()?;

        Ok(Arc::new(Star{
           shell: StarShell::new( lanes, kernel )
        }))
    }
}

pub struct Star
{
    pub shell: StarShell
}

pub struct StarShell
{
   pub kernel: Box<dyn StarKernel>,
   pub lanes: Vec<Lane>
}

impl StarShell
{
   fn new(lanes: Vec<Lane>, kernel: Box<dyn StarKernel>)->Self
   {
       StarShell{
           kernel: kernel,
           lanes: lanes
       }
   }
}


pub trait ProtoStarKernel: Send+Sync
{
    fn default_id(&self) ->Option<Id>;
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