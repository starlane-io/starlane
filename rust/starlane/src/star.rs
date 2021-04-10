use std::sync::{Mutex, Weak, Arc};
use crate::lane::{Lane};
use std::collections::HashMap;
use std::sync::atomic::{AtomicI32, Ordering};
use futures::future::join_all;
use futures::future::select_all;
use crate::message::ProtoGram;
use crate::error::Error;
use crate::id::{Id, IdSeq};
use futures::FutureExt;
use serde::{Serialize,Deserialize};
use crate::proto::ProtoLane;
use std::fmt;

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Serialize, Deserialize)]
pub enum StarKind
{
    Central,
    Mesh,
    Supervisor,
    Server,
    Gateway
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, Serialize, Deserialize)]
pub struct StarKey
{
    pub constellation: Vec<u8>,
    pub index: u16
}

impl fmt::Display for StarKey{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({:?},{})",self.constellation,self.index)
    }
}
impl StarKey
{
   pub fn new( index: u16)->Self
   {
       StarKey {
           constellation: vec![],
           index: index
       }
   }

   pub fn new_with_constellation(constellation: Vec<u8>, index: u16) ->Self
   {
      StarKey {
          constellation,
          index: index
      }
   }

   pub fn with_index( &self, index: u16)->Self
   {
       StarKey {
           constellation: self.constellation.clone(),
           index: index
       }
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
   pub fn new(lanes: Vec<Lane>, kernel: Box<dyn StarKernel>)->Self
   {
       StarShell{
           kernel: kernel,
           lanes: lanes
       }
   }
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
    pub async fn send(&self, gram: ProtoGram) ->Result<(),Error>
    {
        Ok(self.lane.tx.send(gram).await?)
    }

    pub async fn receive( &mut self)->Option<ProtoGram>
    {
        self.lane.rx.recv().await
    }
}


