use crate::id::Id;
use std::fmt;
use crate::star::{StarKey, StarKind};
use serde::{Deserialize, Serialize, Serializer};

#[derive(Clone)]
pub struct Command
{
    pub from: i32,
    pub gram: ProtoFrame
}

#[derive(Clone,Serialize,Deserialize)]
pub enum ProtoFrame
{
    StarLaneProtocolVersion(i32),
    ReportStarKey(StarKey)
}

#[derive(Clone,Serialize,Deserialize)]
pub enum Frame
{
    Proto(ProtoFrame),
    Close,
    Ping,
    Pong,
    RequestSubgraphExpansion,
    GrantSubgraphExpansion(Vec<u16>),
    StarSearch(StarSearchInner),
    StarSearchResult(StarSearchResultInner),
    StarMessage(StarMessageInner)
}

#[derive(Clone,Serialize,Deserialize)]
pub struct StarSearchInner
{
    pub from: StarKey,
    pub pattern: StarSearchPattern,
    pub hops: Vec<StarKey>,
    pub transactions: Vec<i64>,
    pub max_hops: i32,
    pub multi: bool
}

impl StarSearchInner
{
    pub fn inc( &mut self, hop: StarKey, transaction: i64 )
    {
        self.hops.push( hop );
        self.transactions.push(transaction);
    }

}

#[derive(Clone,Serialize,Deserialize)]
pub enum StarSearchPattern
{
    StarKey(StarKey),
    StarKind(StarKind)
}

impl StarSearchPattern
{
    pub fn is_single_match(&self) -> bool
    {
        match self
        {
            StarSearchPattern::StarKey(_) => {true}
            StarSearchPattern::StarKind(_) => {false}
        }
    }
}

#[derive(Clone,Serialize,Deserialize)]
pub struct StarSearchResultInner
{
    pub missed: Option<StarKey>,
    pub hits: Vec<StarSearchHit>,
    pub search: StarSearchInner,
    pub transactions: Vec<i64>,
    pub hops : Vec<StarKey>
}

impl StarSearchResultInner
{
    pub fn pop(&mut self)
    {
        self.transactions.pop();
        self.hops.pop();
    }
}

#[derive(Clone,Serialize,Deserialize,Hash,Eq,PartialEq)]
pub struct StarSearchHit
{
    pub star: StarKey,
    pub hops: i32
}


#[derive(Clone,Serialize,Deserialize)]
pub struct StarMessageInner
{
   pub from: StarKey,
   pub to: StarKey,
   pub transaction: Option<Id>,
   pub payload: StarMessagePayload
}

impl StarMessageInner
{
    pub fn new(from: StarKey, to: StarKey, payload: StarMessagePayload) -> Self
    {
        StarMessageInner {
            from: from,
            to: to,
            transaction: Option::None,
            payload: payload
        }
    }

    pub fn reply(&mut self, payload: StarMessagePayload)
    {
        let tmp = self.from.clone();
        self.from = self.to.clone();
        self.to = tmp;
        self.payload = payload;
    }

}

#[derive(Clone,Serialize,Deserialize)]
pub enum StarMessagePayload
{
   RequestSequence,
   AssignSequence(i64)
}




impl fmt::Display for Frame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = match self {
            Frame::Proto(_) => format!("Proto").to_string(),
            Frame::Close => format!("Close").to_string(),
            Frame::Ping => format!("Ping").to_string(),
            Frame::Pong =>  format!("Pong").to_string(),
            Frame::StarMessage(_)=>format!("StarMessage").to_string(),
            Frame::StarSearch(_)=>format!("StarSearch").to_string(),
            Frame::StarSearchResult(_)=>format!("StarSearchResult").to_string(),
            Frame::RequestSubgraphExpansion=>format!("RequestSubgraphExpansion").to_string(),
            Frame::GrantSubgraphExpansion(_)=>format!("GrantSubgraphExpansion").to_string(),
        };
        write!(f, "{}",r)
    }
}

impl fmt::Display for ProtoFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = match self {
            ProtoFrame::StarLaneProtocolVersion(version) => format!("StarLaneProtocolVersion({})", version).to_string(),
            ProtoFrame::ReportStarKey(id) => format!("ReportStarId({})", id).to_string()
        };
        write!(f, "{}",r)
    }
}