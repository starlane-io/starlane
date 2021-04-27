use crate::id::Id;
use std::fmt;
use crate::star::{StarKey, StarKind, StarWatchInfo};
use serde::{Deserialize, Serialize, Serializer};
use crate::entity::{EntityKey, EntityLocation};
use std::sync::Arc;
use std::collections::HashMap;
use tokio::time::Instant;
use crate::application::{AppLocation, AppKey, AppInfo, AppKind};

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
    StarUnwind(StarUnwindInner),
    Watch(Watch),
    EntityEvent(EntityEvent)
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
    pub entity: EntityKey,
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
    pub transactions: Vec<Id>,
    pub max_hops: usize,
}

impl StarSearchInner
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
pub struct StarSearchResultInner
{
    pub missed: Option<StarKey>,
    pub hits: Vec<SearchHit>,
    pub search: StarSearchInner,
    pub transactions: Vec<Id>,
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
pub struct SearchHit
{
    pub star: StarKey,
    pub hops: usize
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
   ResourceStateRequest(EntityKey),
   ResourceEvent(EntityEvent),
   ResourceMessage(EntityMessage),
   ResourceRequestLocation(ResourceRequestLocation),
   ResourceReportLocation(EntityLocation)
}

#[derive(Clone,Serialize,Deserialize)]
pub struct EntityEvent
{
    pub entity: EntityKey,
    pub kind: ResourceEventKind,
}

#[derive(Clone,Serialize,Deserialize)]
pub enum ResourceEventKind
{
   ResourceStateChange(ResourceState),
   ResourceGathered(ResourceGathered),
   ResourceScattered(ResourceScattered),
   Broadcast(ResourceBroadcast)
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ResourceState
{
    pub payloads: ResourcePayloads
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ResourceGathered
{
    pub to: EntityKey
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ResourceScattered
{
    pub from : EntityKey
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ResourceBroadcast
{
    pub topic: String,
    pub payloads: ResourcePayloads
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ResourcePayloads
{
    pub map: HashMap<String,ResourcePayload>
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ResourcePayload
{
    pub kind: String,
    pub data: Arc<Vec<u8>>
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ResourceReportBind
{
    pub star: StarKey,
    pub key: EntityKey,
    pub name: Option<String>
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ResourceRequestLocation
{
    pub lookup: EntityLookup
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ResourceReportLocation
{
    pub resource: EntityKey,
    pub location: EntityLocation
}


#[derive(Clone,Serialize,Deserialize)]
pub enum EntityLookup
{
    Key(EntityKey),
    Name(ResourceNameLookup)
}

impl EntityLookup
{
    pub fn app_id(&self)->Id
    {
        match self
        {
            EntityLookup::Key(resource) => resource.app.clone(),
            EntityLookup::Name(name) => name.app_id.clone()
        }
    }
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ResourceNameLookup
{
    pub app_id: Id,
    pub name: String
}


pub enum EntityFromKind
{
    Entity(EntityFrom)
}

#[derive(Clone,Serialize,Deserialize)]
pub struct EntityFrom
{
    key: EntityKey,
    source: Option<Vec<u8>>
}

#[derive(Clone,Serialize,Deserialize)]
pub struct EntityTo
{
    key: EntityKey,
    target: Option<Vec<u8>>
}

#[derive(Clone,Serialize,Deserialize)]
pub struct EntityMessage
{
    pub id: Id,
    pub from: EntityFromKind,
    pub to: EntityTo,
    pub payloads: ResourcePayloads,
    pub transaction: Option<Id>
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ResourceBind
{
   pub key: EntityKey,
   pub star: StarKey
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
    pub kind: AppKind,
    pub data: Vec<u8>,
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ApplicationAssignInner
{
    pub app : AppInfo,
    pub data: Vec<u8>,
    pub notify: Vec<StarKey>,
    pub supervisor: StarKey
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ApplicationNotifyReadyInner
{
    pub location: AppLocation
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ApplicationRequestSupervisorInner
{
    pub app: AppKey,
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ApplicationReportSupervisorInner
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
            StarMessagePayload::ApplicationRequestSupervisor(_) => "ApplicationRequestSupervisor".to_string(),
            StarMessagePayload::ApplicationReportSupervisor(_) => "ApplicationReportSupervisor".to_string(),
            StarMessagePayload::ApplicationLookupId(_) => "ApplicationLookupId".to_string(),
            StarMessagePayload::ApplicationRequestLaunch(_) => "ApplicationRequestLaunch".to_string(),
            StarMessagePayload::ServerPledgeToSupervisor => "ServerPledgeToSupervisor".to_string(),
            StarMessagePayload::ResourceEvent(_)=>"ResourceEvent".to_string(),
            StarMessagePayload::ResourceMessage(_)=>"ResourceMessage".to_string(),
            StarMessagePayload::ResourceRequestLocation(_)=>"ResourceRequestLocation".to_string(),
            StarMessagePayload::ResourceReportLocation(_)=>"ResourceReportLocation".to_string(),
            StarMessagePayload::ResourceStateRequest(_)=>"ResourceStateRequest".to_string(),
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
            Frame::Watch(_) => format!("Watch").to_string(),
            Frame::EntityEvent(_) => format!("ResourceEvent").to_string()
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

