use std::sync::{Mutex, Weak, Arc};
use crate::lane::{LaneRunner, TunnelConnector, OutgoingLane, Lane, ConnectorController};
use std::collections::HashMap;
use std::sync::atomic::{AtomicI32};
use futures::future::join_all;
use futures::future::select_all;
use crate::frame::ProtoFrame;
use crate::error::Error;
use crate::id::{Id, IdSeq};
use futures::FutureExt;
use serde::{Serialize,Deserialize};
use crate::proto::ProtoTunnel;
use std::{fmt, cmp};
use tokio::sync::mpsc::Sender;
use std::cmp::Ordering;

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Serialize, Deserialize)]
pub enum StarKind
{
    Central,
    Mesh,
    Supervisor,
    Server,
    Gateway
}

#[derive(PartialEq, Eq, PartialOrd, Hash, Debug, Clone, Serialize, Deserialize)]
pub struct StarKey
{
    pub constellation: Vec<u8>,
    pub index: u16
}

impl cmp::Ord for StarKey
{
    fn cmp(&self, other: &Self) -> Ordering {
        if self.constellation.len() > other.constellation.len()
        {
            Ordering::Greater
        }
        else if self.constellation.len() < other.constellation.len()
        {
            Ordering::Less
        }
        else if self.constellation.cmp(&other.constellation ) != Ordering::Equal
        {
            return self.constellation.cmp(&other.constellation );
        }
        else
        {
            return self.index.cmp(&other.index );
        }
    }
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

   // highest to lowest
   pub fn sort( a : StarKey, b: StarKey  ) -> Result<(Self,Self),Error>
   {
       if a == b
       {
           Err(format!("both StarKeys are equal. {}=={}",a,b).into())
       }
       else if a.cmp(&b) == Ordering::Greater
       {
           Ok((a,b))
       }
       else
       {
           Ok((b,a))
       }
   }
}


pub struct Star
{
   pub kernel: Box<dyn StarKernel>,
   pub lanes: HashMap<StarKey, LaneRunner>
}

impl Star
{
   pub fn new(lanes: HashMap<StarKey, LaneRunner>, kernel: Box<dyn StarKernel>) ->Self
   {
       Star{
           kernel: kernel,
           lanes: lanes
       }
   }
}




pub trait StarKernel : Send
{
}

pub struct LaneMeta
{
   pub id: i32,
   pub lane: LaneRunner
}

impl LaneMeta
{
    pub async fn send(&self, gram: ProtoFrame) ->Result<(),Error>
    {
        //Ok(self.lane.tunnel_tx.send(gram).await?)
        unimplemented!()
    }

    pub async fn receive( &mut self)->Option<ProtoFrame>
    {
        //self.lane.tunnel_rx.recv().await
        unimplemented!()
    }
}

pub enum StarCommand
{
    AddLane(Lane),
    AddConnectorController(ConnectorController)
}

#[derive(Clone)]
pub struct StarController
{
    pub command_tx: Sender<StarCommand>
}


