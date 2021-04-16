use crate::id::Id;
use std::fmt;
use crate::star::{StarKey, StarKind};
use serde::{Deserialize, Serialize, Serializer};

#[derive(Clone)]
pub struct Command
{
    pub from: i32,
    pub frame: ProtoFrame
}

#[derive(Clone,Serialize,Deserialize)]
pub enum ProtoFrame
{
    StarLaneProtocolVersion(i32),
    ReportStarKey(StarKey),
    RequestSubgraphExpansion,
    GrantSubgraphExpansion(Vec<u16>),
    CentralSearch,
    CentralFound(usize)
}

#[derive(Clone,Serialize,Deserialize)]
pub enum Frame
{
    Close,
    Proto(ProtoFrame),
    Diagnose(FrameDiagnose),
    StarSearch(StarSearchInner),
    StarSearchResult(StarSearchResultInner),
    StarMessage(StarMessageInner),
    StarAck(StarAckInner),
    StarWind(StarWindInner),
    StarUnwind(StarUnwindInner)
}

#[derive(Clone,Serialize,Deserialize)]
pub struct StarAckInner
{
    to: StarKey,
    id: Id
}

#[derive(Clone,Serialize,Deserialize)]
pub enum StarWindPayload
{
    RequestSequence
}

#[derive(Clone,Serialize,Deserialize)]
pub enum StarUnwindPayload
{
    AssignSequence(i64)
}

#[derive(Clone,Serialize,Deserialize)]
pub struct StarWindInner
{
  pub to: StarKey,
  pub stars: Vec<StarKey>,
  pub payload: StarWindPayload
}

#[derive(Clone,Serialize,Deserialize)]
pub struct StarUnwindInner
{
    pub stars: Vec<StarKey>,
    pub payload: StarUnwindPayload
}

#[derive(Clone,Serialize,Deserialize)]
pub enum FrameDiagnose
{
  Ping,
  Pong,
}


#[derive(Clone,Serialize,Deserialize)]
pub struct StarSearchInner
{
    pub from: StarKey,
    pub pattern: StarSearchPattern,
    pub hops: Vec<StarKey>,
    pub transactions: Vec<i64>,
    pub max_hops: i32,
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
   pub id: Id,
   pub transaction: Option<Id>,
   pub payload: StarMessagePayload,
   pub retry: usize,
   pub max_retries: usize
}

impl StarMessageInner
{
    pub fn new(id:Id, from: StarKey, to: StarKey, payload: StarMessagePayload) -> Self
    {
        StarMessageInner {
            id: id,
            from: from,
            to: to,
            transaction: Option::None,
            payload: payload,
            retry: 0,
            max_retries: 16
        }
    }

    pub fn to_central(id:Id, from: StarKey, payload: StarMessagePayload ) -> Self
    {
        StarMessageInner {
            id: id,
            from: from,
            to: StarKey::central(),
            transaction: Option::None,
            payload: payload,
            retry: 0,
            max_retries: 16
        }
    }


    pub fn reply(&mut self, id:Id, payload: StarMessagePayload)
    {
        let tmp = self.from.clone();
        self.from = self.to.clone();
        self.to = tmp;
        self.payload = payload;
        self.retry = 0;
    }

    pub fn inc_retry(&mut self)
    {
        self.retry = self.retry + 1;
    }

}

#[derive(Clone,Serialize,Deserialize)]
pub enum StarMessagePayload
{
   Reject(RejectionInner),
   SupervisorPledgeToCentral,
   ApplicationCreateRequest(ApplicationCreateRequestInner),
   ApplicationAssign(ApplicationAssignInner),
   ApplicationNotifyReady(ApplicationNotifyReadyInner),
   ApplicationRequestSupervisor(ApplicationRequestSupervisorInner),
   ApplicationReportSupervisor(ApplicationReportSupervisorInner),
   ApplicationLookupId(ApplicationLookupIdInner),
   ApplicationRequestLaunch(ApplicationRequestLaunchInner),
   ServerPledgeToSupervisor,
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ApplicationRequestLaunchInner
{
    pub app_id: Id,
    pub data: Vec<u8>
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

impl fmt::Display for FrameDiagnose {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = match self {
            FrameDiagnose::Ping => "Ping",
            FrameDiagnose::Pong => "Pong"
        };
        write!(f, "{}",r)
    }
}

impl fmt::Display for StarMessagePayload{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = match self {
            StarMessagePayload::Reject(inner) => format!("Reject({})",inner.message),
            StarMessagePayload::SupervisorPledgeToCentral => "SupervisorPledgeToCentral".to_string(),
            StarMessagePayload::ApplicationCreateRequest(_) => "ApplicationCreateRequest".to_string(),
            StarMessagePayload::ApplicationAssign(_) => "ApplicationAssign".to_string(),
            StarMessagePayload::ApplicationNotifyReady(_) => "ApplicationNotifyReady".to_string(),
            StarMessagePayload::ApplicationRequestSupervisor(_) => "ApplicationRequestSupervisor".to_string(),
            StarMessagePayload::ApplicationReportSupervisor(_) => "ApplicationReportSupervisor".to_string(),
            StarMessagePayload::ApplicationLookupId(_) => "ApplicationLookupId".to_string(),
            StarMessagePayload::ApplicationRequestLaunch(_) => "ApplicationRequestLaunch".to_string(),
            StarMessagePayload::ServerPledgeToSupervisor => "ServerPledgeToSupervisor".to_string()
        };
        write!(f, "{}",r)
    }
}


impl fmt::Display for Frame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = match self {
            Frame::Proto(proto) => format!("Proto({})",proto).to_string(),
            Frame::Close => format!("Close").to_string(),
            Frame::Diagnose(diagnose)=> format!("Diagnose({})",diagnose).to_string(),
            Frame::StarMessage(inner)=>format!("StarMessage({})",inner.payload).to_string(),
            Frame::StarSearch(_)=>format!("StarSearch").to_string(),
            Frame::StarSearchResult(_)=>format!("StarSearchResult").to_string(),
            Frame::StarWind(_)=>format!("StarWind").to_string(),
            Frame::StarUnwind(_)=>format!("StarUnwind").to_string(),
            Frame::StarAck(_)=>format!("StarAck").to_string(),
        };
        write!(f, "{}",r)
    }
}

impl fmt::Display for ProtoFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = match self {
            ProtoFrame::StarLaneProtocolVersion(version) => format!("StarLaneProtocolVersion({})", version).to_string(),
            ProtoFrame::ReportStarKey(id) => format!("ReportStarId({})", id).to_string(),
            ProtoFrame::RequestSubgraphExpansion=> format!("RequestSubgraphExpansion").to_string(),
            ProtoFrame::GrantSubgraphExpansion(path) => format!("GrantSubgraphExpansion({:?})", path).to_string(),
            ProtoFrame::CentralFound(_) => format!("CentralFound").to_string(),
            ProtoFrame::CentralSearch => format!("CentralSearch").to_string(),
        };
        write!(f, "{}",r)
    }
}