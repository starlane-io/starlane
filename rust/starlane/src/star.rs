use std::sync::{Mutex, Weak, Arc};
use crate::lane::{Lane, TunnelConnector, OutgoingLane, ConnectorController, LaneMeta};
use std::collections::HashMap;
use std::sync::atomic::{AtomicI32};
use futures::future::join_all;
use futures::future::select_all;
use crate::frame::{ProtoFrame, Frame, StarSearchHit, StarSearchPattern};
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

impl StarKey
{
    pub fn central()->Self
    {
        StarKey{
            constellation: vec![],
            index: 0
        }
    }
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
   pub lanes: HashMap<StarKey, LaneMeta>
}

impl Star
{
   pub fn new(lanes: HashMap<StarKey, LaneMeta>, kernel: Box<dyn StarKernel>) ->Self
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





pub enum StarCommand
{
    AddLane(Lane),
    AddConnectorController(ConnectorController),
    Frame(Frame)
}

impl fmt::Display for StarCommand{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = match self {
            StarCommand::AddLane(_) => format!("AddLane").to_string(),
            StarCommand::AddConnectorController(_) => format!("AddConnectorController").to_string(),
            StarCommand::Frame(frame) => format!("Frame({})",frame).to_string(),
        };
        write!(f, "{}",r)
    }
}

#[derive(Clone)]
pub struct StarController
{
    pub command_tx: Sender<StarCommand>
}


pub struct StarSearchTransaction
{
    pub pattern: StarSearchPattern,
    pub reported_lane_count: i32,
    pub hits: HashMap<StarKey,StarSearchHit>
}

impl StarSearchTransaction
{
    pub fn new(pattern: StarSearchPattern) ->Self
    {
        StarSearchTransaction{
            pattern: pattern,
            reported_lane_count: 0,
            hits: HashMap::new()
        }
    }
}


pub enum TransactionState
{
    Continue,
    Done
}

pub trait Transaction : Send
{
    fn on_frame( &mut self, frame: Frame, lane: & mut LaneMeta )->TransactionState;
}

pub struct StarKeySearchTransaction
{
}

impl Transaction for StarKeySearchTransaction
{
    fn on_frame(&mut self, frame: Frame, lane: &mut LaneMeta)->TransactionState {

        if let Frame::StarSearchResult(result) = frame
        {
            for hit in result.hits
            {
                lane.star_paths.insert(hit.star.clone());
            }

            if let Option::Some(missed) = result.missed
            {
                lane.not_star_paths.insert(missed);
            }
        }

        TransactionState::Done
    }
}
