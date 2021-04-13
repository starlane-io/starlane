use std::sync::{Mutex, Weak, Arc};
use crate::lane::{Lane, TunnelConnector, OutgoingLane, ConnectorController, LaneMeta, LaneCommand, TunnelConnectorFactory, ConnectionInfo};
use std::collections::HashMap;
use std::sync::atomic::{AtomicI32, AtomicI64 };
use futures::future::join_all;
use futures::future::select_all;
use crate::frame::{ProtoFrame, Frame, StarSearchHit, StarSearchPattern, StarMessageInner, StarMessagePayload, StarSearchInner, StarSearchResultInner};
use crate::error::Error;
use crate::id::{Id, IdSeq};
use futures::FutureExt;
use serde::{Serialize,Deserialize};
use crate::proto::{ProtoTunnel, ProtoStar, PlaceholderKernel};
use std::{fmt, cmp};
use tokio::sync::mpsc::{Sender, Receiver};
use std::cmp::Ordering;
use tokio::sync::mpsc;
use crate::frame::Frame::{StarSearch, StarMessage};
use url::Url;

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Serialize, Deserialize)]
pub enum StarKind
{
    Central,
    Mesh,
    Supervisor,
    Server,
    Gateway,
    Link,
    Client,
    Ext(ExtStarKind)
}



impl StarKind
{
    pub fn is_central(&self)->bool
    {
        if let StarKind::Central = self
        {
            return true;
        }
        else {
            return false;
        }
    }

    pub fn is_supervisor(&self)->bool
    {
        if let StarKind::Supervisor = self
        {
            return true;
        }
        else {
            return false;
        }
    }


    pub fn is_client(&self)->bool
    {
        if let StarKind::Client = self
        {
            return true;
        }
        else {
            return false;
        }
    }


    pub fn relay(&self) ->bool
    {
        match self
        {
            StarKind::Central => false,
            StarKind::Mesh => true,
            StarKind::Supervisor => false,
            StarKind::Server => true,
            StarKind::Gateway => true,
            StarKind::Client => true,
            StarKind::Link => true,
            StarKind::Ext(ext) => ext.relay_messages,
        }
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Serialize, Deserialize)]
pub enum StarData
{
    Central,
    Mesh,
    Supervisor,
    Server,
    Gateway(ServiceData),
    Link(ConnectionInfo),
    Client,
    Ext
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Serialize, Deserialize)]
pub struct ServiceData
{
    pub port: u16
}


#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Serialize, Deserialize)]
pub struct GatewayKind
{

}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Serialize, Deserialize)]
pub struct ExtStarKind
{
    relay_messages: bool
}




#[derive(PartialEq, Eq, PartialOrd, Hash, Debug, Clone, Serialize, Deserialize)]
pub struct StarKey
{
    pub subgraph: Vec<u16>,
    pub index: u16
}

impl StarKey
{
    pub fn central()->Self
    {
        StarKey{
            subgraph: vec![],
            index: 0
        }
    }
}

impl cmp::Ord for StarKey
{
    fn cmp(&self, other: &Self) -> Ordering {
        if self.subgraph.len() > other.subgraph.len()
        {
            Ordering::Greater
        }
        else if self.subgraph.len() < other.subgraph.len()
        {
            Ordering::Less
        }
        else if self.subgraph.cmp(&other.subgraph) != Ordering::Equal
        {
            return self.subgraph.cmp(&other.subgraph);
        }
        else
        {
            return self.index.cmp(&other.index );
        }
    }
}

impl fmt::Display for StarKey{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({:?},{})", self.subgraph, self.index)
    }
}
impl StarKey
{
   pub fn new( index: u16)->Self
   {
       StarKey {
           subgraph: vec![],
           index: index
       }
   }

   pub fn new_with_subgraph(subgraph: Vec<u16>, index: u16) ->Self
   {
      StarKey {
          subgraph,
          index: index
      }
   }

   pub fn with_index( &self, index: u16)->Self
   {
       StarKey {
           subgraph: self.subgraph.clone(),
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



pub static MAX_HOPS: i32 = 32;

pub struct Star
{
    pub kind: StarKind,
    pub key: StarKey,
    command_rx: Receiver<StarCommand>,
    lanes: HashMap<StarKey, LaneMeta>,
    connector_ctrls: Vec<ConnectorController>,
    sequence: Option<IdSeq>,
    transactions: HashMap<i64,Box<dyn Transaction>>,
    transaction_seq: AtomicI64,
    star_search_transactions: HashMap<i64,StarSearchTransaction>,
    frame_hold: HashMap<StarKey,Vec<Frame>>
}

impl Star
{
    pub fn new(key: StarKey, kind: StarKind) ->(Self, StarController)
    {
        let (command_tx, command_rx) = mpsc::channel(32);
        (Star{
            kind,
            key,
            command_rx: command_rx,
            lanes: HashMap::new(),
            connector_ctrls: vec![],
            sequence: Option::None,
            transactions: HashMap::new(),
            transaction_seq: AtomicI64::new(0),
            star_search_transactions: HashMap::new(),
            frame_hold: HashMap::new(),
        }, StarController{
            command_tx: command_tx
        })
    }

    pub fn from_proto(key: StarKey, kind: StarKind, command_rx: Receiver<StarCommand>, lanes: HashMap<StarKey,LaneMeta>, connector_ctrls: Vec<ConnectorController>) ->Self
    {
        Star{
            kind,
            key,
            command_rx: command_rx,
            lanes: lanes,
            connector_ctrls: connector_ctrls,
            sequence: Option::None,
            transactions: HashMap::new(),
            transaction_seq: AtomicI64::new(0),
            star_search_transactions: HashMap::new(),
            frame_hold: HashMap::new(),
        }
    }


    pub async fn run(mut self)
    {
        // request a sequence from central
        loop {
            let mut futures = vec!();
            let mut lanes = vec!();
            futures.push(self.command_rx.recv().boxed() );

            for (key,mut lane) in &mut self.lanes
            {
                futures.push( lane.lane.incoming.recv().boxed() );
                lanes.push( key.clone() )
            }

            let (command,index,_) = select_all(futures).await;

            if let Some(command) = command
            {
                match command{
                    StarCommand::AddLane(lane) => {
                        if let Some(remote_star)=lane.remote_star.as_ref()
                        {
                            self.lanes.insert(remote_star.clone(), LaneMeta::new(lane));
                        }
                        else {
                            eprintln!("for star remote star must be set");
                         }
                    }
                    StarCommand::AddConnectorController(connector_ctrl) => {
                        self.connector_ctrls.push(connector_ctrl);
                    }
                    StarCommand::Frame(frame) => {
                        let lane_key = lanes.get(index-1).unwrap().clone();
                        self.process_frame(frame, lane_key );
                    }
                }
            }
            else
            {
                println!("command_rx has been disconnected");
                return;
            }

        }

    }

    async fn lane_added(&mut self)
    {
        if self.sequence.is_none()
        {
            let message = Frame::StarMessage(StarMessageInner{
                from: self.key.clone(),
                to: StarKey::central(),
                transaction: None,
                payload: StarMessagePayload::RequestSequence
            });
            self.send(&StarKey::central(), message).await
        }
    }

    async fn send(&mut self, star: &StarKey, frame: Frame )
    {
        for (remote_star,lane) in &self.lanes
        {
            if lane.has_path_to_star(star)
            {
                lane.lane.outgoing.tx.send( LaneCommand::LaneFrame(frame) ).await;
                return;
            }
        }
        if let None = self.frame_hold.get(star)
        {
            self.frame_hold.insert(star.clone(), vec![] );
        }
        if let Some(frames) = self.frame_hold.get_mut(star)
        {
            frames.push(frame);
        }
        self.search_for_star(star.clone());
    }

    async fn search_for_star(&mut self, star: StarKey )
    {
        let search_id = self.transaction_seq.fetch_add(1, std::sync::atomic::Ordering::Relaxed );
        let search_transaction = StarSearchTransaction::new(StarSearchPattern::StarKey(self.key.clone()));
        self.star_search_transactions.insert(search_id, search_transaction );

        let search = Frame::StarSearch(StarSearchInner{
            from: self.key.clone(),
            pattern: StarSearchPattern::StarKey(star),
            hops: vec![self.key.clone()],
            transactions: vec![search_id],
            max_hops: MAX_HOPS,
            multi: false
        });

        for (star,lane) in &self.lanes
        {
            lane.lane.outgoing.tx.send( LaneCommand::LaneFrame(search.clone())).await;
        }
    }

    async fn process_frame( &mut self, frame: Frame, lane_key: StarKey )
    {
        match frame
        {
            StarSearch(search) => {
                self.on_star_search(search, lane_key).await;
            }
            Frame::StarSearchResult(result) => {
                self.on_star_search_result(result, lane_key ).await;
            }
            StarMessage(message) => {
                match self.on_message(message).await
                {
                    Ok(_) => {}
                    Err(error) => {

                    }
                }
            }
            _ => {
                eprintln!("star does not handle frame: {}", frame)
            }
        }
    }

    async fn on_star_search( &mut self, mut search: StarSearchInner, lane_key: StarKey )
    {
        let hit = match &search.pattern
        {
            StarSearchPattern::StarKey(star) => {
                self.key == *star
            }
            StarSearchPattern::StarKind(kind) => {
                self.kind == *kind
            }
        };

        if hit
        {
            if search.pattern.is_single_match()
            {
                let hops = search.hops.len() + 1;
                let frame = Frame::StarSearchResult( StarSearchResultInner {
                    missed: None,
                    hops: search.hops.clone(),
                    hits: vec![ StarSearchHit { star: self.key.clone(), hops: hops as _ } ],
                    search: search.clone(),
                    transactions: search.transactions.clone()
                });

                let lane = self.lanes.get_mut(&lane_key).unwrap();
                lane.lane.outgoing.tx.send(LaneCommand::LaneFrame(frame)).await;
            }
            else {
                // create a SearchTransaction here.
                // gather ALL results into this transaction
            }

            if !search.multi
            {
                return;
            }
        }

        let search_id = self.transaction_seq.fetch_add(1,std::sync::atomic::Ordering::Relaxed);
        let search_transaction = StarSearchTransaction::new(search.pattern.clone() );
        self.star_search_transactions.insert(search_id,search_transaction);

        search.inc( self.key.clone(), search_id );

        if search.max_hops > MAX_HOPS
        {
            eprintln!("rejecting a search with more than 255 hops");
        }

        if (search.hops.len() as i32) > search.max_hops || self.lanes.len() <= 1
        {
            eprintln!("search has reached maximum hops... need to send not found");
        }

        for (star,lane) in &self.lanes
        {
            if !search.hops.contains(star)
            {
                lane.lane.outgoing.tx.send(LaneCommand::LaneFrame(Frame::StarSearch(search.clone()))).await;
            }
        }
    }

    async fn on_star_search_result( &mut self, mut search_result: StarSearchResultInner, lane_key: StarKey )
    {
        if let Some(search_id) = search_result.transactions.last()
        {
            if let Some(search_trans) = self.star_search_transactions.get_mut(search_id)
            {
                for hit in &search_result.hits
                {
                    search_trans.hits.insert( hit.star.clone(), hit.clone() );
                    let lane = self.lanes.get_mut(&lane_key).unwrap();
                    lane.star_paths.insert( hit.star.clone() );
                    if let Some(frames) = self.frame_hold.remove( &hit.star )
                    {
                        for frame in frames
                        {
                            lane.lane.outgoing.tx.send( LaneCommand::LaneFrame(frame) ).await;
                        }
                    }
                }
                search_trans.reported_lane_count = search_trans.reported_lane_count+1;

                if search_trans.reported_lane_count >= (self.lanes.len() as i32)-1
                {
                    // this means all lanes have been searched and the search result can be reported to the next node
                    if let Some(search_trans) = self.star_search_transactions.remove(search_id)
                    {
                        search_result.pop();
                        if let Some(next)=search_result.hops.last()
                        {
                            if let Some(lane)=self.lanes.get_mut(next)
                            {
                                search_result.hits = search_trans.hits.values().map(|a|a.clone()).collect();
                                lane.lane.outgoing.tx.send( LaneCommand::LaneFrame(Frame::StarSearchResult(search_result))).await;
                            }
                        }
                    }
                }
            }
        }
    }

    async fn on_message( &mut self, mut message: StarMessageInner ) -> Result<(),Error>
    {
        if message.to != self.key
        {
            if self.kind.relay()
            {
                self.send(&message.to.clone(), Frame::StarMessage(message)).await;
                return Ok(());
            }
            else {
                return Err("this star does not relay messages".into())
            }
        }
        else {

            match message.payload
            {
                StarMessagePayload::RequestSequence => {
                    let sequence = self.sequence.as_ref().unwrap().next().index;
                    message.reply(StarMessagePayload::AssignSequence(sequence));
                    self.send_message( message ).await;
                }
                StarMessagePayload::AssignSequence(sequence) => {
                    self.sequence = Option::Some(IdSeq::new(sequence));
                }
            }

        }
        return Ok(());
    }

    async fn send_message( &mut self, message: StarMessageInner )
    {
        self.send( &message.to.clone(), Frame::StarMessage(message) );
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
