use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use serde::{Deserialize, Serialize, Serializer};
use tokio::time::Instant;

use crate::actor::{ActorKey, ActorLocation};
use crate::app::{AppInfo, AppKey, AppKind, AppLocation};
use crate::id::Id;
use crate::star::{StarKey, StarKind, StarWatchInfo};
use crate::user::{User, UserKey, GroupKey};
use crate::org::OrgKey;

#[derive(Clone,Serialize,Deserialize)]
pub enum Frame
{
    Close,
    Proto(ProtoFrame),
    Diagnose(FrameDiagnose),
    StarSearch(StarSearch),
    StarSearchResult(StarSearchResult),
    StarMessage(StarMessage),
    StarMessageAck(StarMessageAck),
    StarWind(StarWind),
    StarUnwind(StarUnwind),
    Watch(Watch),
    Event(Event)
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
pub enum Watch
{
    Add(WatchInfo),
    Remove(WatchInfo)
}

#[derive(Clone,Serialize,Deserialize)]
pub struct WatchInfo
{
    pub id: Id,
    pub entity: ActorKey,
}

#[derive(Clone,Serialize,Deserialize)]
pub struct StarMessageAck
{
    pub from: StarKey,
    pub to: StarKey,
    pub id: Id
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
pub struct StarWind
{
  pub to: StarKey,
  pub stars: Vec<StarKey>,
  pub payload: StarWindPayload
}

#[derive(Clone,Serialize,Deserialize)]
pub struct StarUnwind
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
pub struct StarSearch
{
    pub from: StarKey,
    pub pattern: StarSearchPattern,
    pub hops: Vec<StarKey>,
    pub transactions: Vec<Id>,
    pub max_hops: usize,
}

impl StarSearch
{
    pub fn inc( &mut self, hop: StarKey, transaction: Id )
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
pub struct StarSearchResult
{
    pub missed: Option<StarKey>,
    pub hits: Vec<SearchHit>,
    pub search: StarSearch,
    pub transactions: Vec<Id>,
    pub hops : Vec<StarKey>
}

impl StarSearchResult
{
    pub fn pop(&mut self)
    {
        self.transactions.pop();
        self.hops.pop();
    }
}

#[derive(Clone,Serialize,Deserialize,Hash,Eq,PartialEq)]
pub struct SearchHit
{
    pub star: StarKey,
    pub hops: usize
}


#[derive(Clone,Serialize,Deserialize)]
pub struct StarMessage
{
   pub from: StarKey,
   pub to: StarKey,
   pub id: Id,
   pub transaction: Option<Id>,
   pub payload: StarMessagePayload,
   pub retry: usize,
   pub max_retries: usize
}

impl StarMessage
{
    pub fn new(id:Id, from: StarKey, to: StarKey, payload: StarMessagePayload) -> Self
    {
        StarMessage {
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
        StarMessage {
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
        self.retry = &self.retry + 1;
    }

}

#[derive(Clone,Serialize,Deserialize)]
pub enum StarMessagePayload
{
   Pledge,
   OrgMessage(OrgMessage),
   Ok,
   Error(String),
   Reject(Rejection),
}

#[derive(Clone,Serialize,Deserialize)]
pub struct OrgMessage
{
    pub org: OrgKey,
    pub group: GroupKey,
    pub user: UserKey,
    pub payload: OrgMessagePayload
}

#[derive(Clone,Serialize,Deserialize)]
pub enum OrgMessagePayload
{
    App(AppMessage),
    Actor(ActorMessage),
    Location(LocationMessage)
}


#[derive(Clone,Serialize,Deserialize)]
pub enum LocationMessage
{
    Request(LocationRequest),
    Report(LocationReport)
}

#[derive(Clone,Serialize,Deserialize)]
pub enum LocationRequest
{
    App(AppKey),
    Actor(ActorKey)
}

#[derive(Clone,Serialize,Deserialize)]
pub enum LocationResponse
{
    NotFound,
    App(StarKey),
    Actor(StarKey)
}




#[derive(Clone,Serialize,Deserialize)]
pub struct AppMessage
{
    pub app: AppKey,
    pub payload: AppMessagePayload
}

#[derive(Clone,Serialize,Deserialize)]
pub enum AppMessagePayload
{
    ApplicationCreateRequest(ApplicationCreateRequest),
    ApplicationAssign(ApplicationAssign),
    ApplicationNotifyReady(ApplicationNotifyReady),
    ApplicationSupervisorRequest(ApplicationSupervisorRequest),
    ApplicationSupervisorReport(ApplicationSupervisorReport),
    ApplicationLookup(ApplicationLookup),
}

#[derive(Clone,Serialize,Deserialize)]
pub enum Event
{
    Actor(ActorEvent),
    Star(StarEvent)
}

#[derive(Clone,Serialize,Deserialize)]
pub enum ActorEvent
{
   StateChange(ActorState),
   Gathered(ActorGathered),
   Scattered(ActorScattered),
   Broadcast(ActorBroadcast)
}

#[derive(Clone,Serialize,Deserialize)]
pub enum StarEvent
{
    Lane(LaneEvent)
}

#[derive(Clone,Serialize,Deserialize)]
pub struct LaneEvent
{
    pub star: StarKey,
    pub kind: LaneEventKind
}

#[derive(Clone,Serialize,Deserialize)]
pub enum LaneEventKind
{
    Connect,
    Disconnect
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ActorState
{
    pub payloads: ActorPayloads
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ActorGathered
{
    pub to: ActorKey
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ActorScattered
{
    pub from : ActorKey
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ActorBroadcast
{
    pub topic: String,
    pub payloads: ActorPayloads
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ActorPayloads
{
    pub map: HashMap<String, ActorPayload>
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ActorPayload
{
    pub kind: String,
    pub data: Arc<Vec<u8>>
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ActorBindReport
{
    pub star: StarKey,
    pub key: ActorKey,
    pub name: Option<String>
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ActorLocationRequest
{
    pub lookup: ActorLookup
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ActorLocationReport
{
    pub resource: ActorKey,
    pub location: ActorLocation
}


#[derive(Clone,Serialize,Deserialize)]
pub enum ActorLookup
{
    Key(ActorKey),
    Name(ActorNameLookup)
}

impl ActorLookup
{
    pub fn app_id(&self)->Id
    {
        match self
        {
            ActorLookup::Key(resource) => resource.app.clone(),
            ActorLookup::Name(name) => name.app_id.clone()
        }
    }
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ActorNameLookup
{
    pub app_id: Id,
    pub name: String
}


#[derive(Clone,Serialize,Deserialize)]
pub enum ActorFromKind
{
    Actor(ActorFrom),
    User(User)
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ActorFrom
{
    key: ActorKey,
    source: Option<Vec<u8>>
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ActorTo
{
    key: ActorKey,
    target: Option<Vec<u8>>
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ActorMessage
{
    pub id: Id,
    pub from: ActorFromKind,
    pub to: ActorTo,
    pub payloads: ActorPayloads,
    pub transaction: Option<Id>
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ActorBind
{
   pub key: ActorKey,
   pub star: StarKey
}


#[derive(Clone,Serialize,Deserialize)]
pub struct ApplicationLaunchRequest
{
    pub app_id: Id,
    pub data: Vec<u8>
}


#[derive(Clone,Serialize,Deserialize)]
pub struct Rejection
{
    pub message: String,
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ApplicationLookup
{
    pub name: String
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ApplicationCreateRequest
{
    pub name: Option<String>,
    pub kind: AppKind,
    pub data: Vec<u8>,
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ApplicationAssign
{
    pub app : AppInfo,
    pub data: Vec<u8>,
    pub notify: Vec<StarKey>,
    pub supervisor: StarKey
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ApplicationNotifyReady
{
    pub location: AppLocation
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ApplicationSupervisorRequest
{
    pub app: AppKey,
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ApplicationSupervisorReport
{
    pub app: AppKey,
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
            StarMessagePayload::ApplicationSupervisorRequest(_) => "ApplicationRequestSupervisor".to_string(),
            StarMessagePayload::ApplicationSupervisorReport(_) => "ApplicationReportSupervisor".to_string(),
            StarMessagePayload::ApplicationLookup(_) => "ApplicationLookupId".to_string(),
            StarMessagePayload::ApplicationLaunchRequest(_) => "ApplicationRequestLaunch".to_string(),
            StarMessagePayload::ServerPledgeToSupervisor => "ServerPledgeToSupervisor".to_string(),
            StarMessagePayload::ActorEvent(_)=>"ActorEvent".to_string(),
            StarMessagePayload::ActorMessage(_)=>"ActorMessage".to_string(),
            StarMessagePayload::ActorLocationRequest(_)=>"ActorLocationRequest".to_string(),
            StarMessagePayload::ActorLocationReport(_)=>"ActorLocationReport".to_string(),
            StarMessagePayload::ActorStateRequest(_)=>"ActorStateRequest".to_string(),
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
            Frame::StarMessageAck(_)=>format!("StarMessageAck").to_string(),
            Frame::Watch(_) => format!("Watch").to_string(),
            Frame::Event(_) => format!("ActorEvent").to_string()
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

