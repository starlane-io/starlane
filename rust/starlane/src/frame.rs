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

    pub fn to_central(from: StarKey, payload: StarMessagePayload) -> Self
    {
        StarMessageInner {
            from: from,
            to: StarKey::central(),
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
   AssignSequence(i64),
   SupervisorPledgeToCentral,
   ApplicationCreateRequest(ApplicationCreateRequestInner),
   ApplicationAssign(ApplicationAssignInner),
   ApplicationNotifyReady(ApplicationNotifyReadyInner),
   ApplicationRequestSupervisor(ApplicationRequestSupervisorInner),
   ApplicationReportSupervisor(ApplicationReportSupervisorInner),
   ApplicationLookupId(ApplicationLookupIdInner),
   ServerPledgeToSupervisor,
   Reject(RejectionInner)
}

#[derive(Clone,Serialize,Deserialize)]
pub struct RejectionInner
{
    pub message: String,
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ApplicationLookupIdInner
{
    pub name: String
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ApplicationCreateRequestInner
{
    pub name: Option<String>,
    pub data: Vec<u8>,
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ApplicationAssignInner
{
    pub app_id: Id,
    pub data: Vec<u8>,
    pub notify: Vec<StarKey>
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ApplicationNotifyReadyInner
{
    pub app_id: Id,
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ApplicationRequestSupervisorInner
{
    pub app_id: Id,
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ApplicationReportSupervisorInner
{
    pub app_id: Id,
    pub supervisor: StarKey
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