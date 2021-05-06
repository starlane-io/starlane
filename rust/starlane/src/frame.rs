use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use serde::{Deserialize, Serialize, Serializer};
use tokio::time::Instant;

use crate::actor::{ActorKey, ActorLocation};
use crate::id::Id;
use crate::star::{StarKey, StarKind, StarWatchInfo, StarNotify, Star, StarCommand, StarInfo};
use crate::label::Labels;
use crate::message::{MessageResult, ProtoMessage, MessageExpect, MessageUpdate};
use tokio::sync::{oneshot, broadcast, mpsc};
use crate::keys::{AppKey, UserKey, SubSpaceKey};
use crate::app::{AppLocation, AppKind, AppInfo};
use crate::user::AuthToken;
use crate::logger::Flags;
use crate::error::Error;

#[derive(Clone,Serialize,Deserialize)]
pub enum Frame
{
    Close,
    Proto(ProtoFrame),
    Diagnose(FrameDiagnose),
    StarWind(StarWind),
//    StarSearchResult(StarSearchResult),
    StarMessage(StarMessage),
    Watch(Watch),
    Event(Event)
}

#[derive(Clone,Serialize,Deserialize)]
pub enum StarWind
{
    Up(WindUp),
    Down(WindDown)
}

#[derive(Clone,Serialize,Deserialize)]
pub enum ProtoFrame
{
    StarLaneProtocolVersion(i32),
    ReportStarKey(StarKey),
    RequestSubgraphExpansion,
    GrantSubgraphExpansion(Vec<u16>),
    CentralSearch,
    CentralFound(usize),
    Evolution(ProtoEvolution),
    Sequence(ProtoSequence),
}

#[derive(Clone,Serialize,Deserialize)]
pub enum ProtoEvolution
{
    Request,
    Report
}

#[derive(Clone,Serialize,Deserialize)]
pub enum ProtoSequence
{
    Request,
    Reply(u64)
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
pub enum FrameDiagnose
{
  Ping,
  Pong,
}


#[derive(Clone,Serialize,Deserialize)]
pub struct WindUp
{
    pub from: StarKey,
    pub pattern: StarPattern,
    pub hops: Vec<StarKey>,
    pub transactions: Vec<Id>,
    pub max_hops: usize,
    pub action: WindAction
}

impl WindUp
{
    pub fn new( from: StarKey, pattern: StarPattern, action: WindAction )->Self
    {
        WindUp
        {
           from: from,
           pattern: pattern,
           action: action,
           hops: vec![],
           transactions: vec![],
           max_hops: 255
        }
    }
}

#[derive(Clone,Serialize,Deserialize)]
pub enum WindAction
{
    SearchHits,
    Flags(Flags)
}

impl WindAction
{
    pub fn update(&self, mut new_hits: Vec<WindHit>, result: WindResults) -> Result<WindResults,Error>
    {
        match self
        {
            WindAction::SearchHits => {
                if let WindResults::None = result {
                    let mut hits = vec!();
                    hits.append( &mut new_hits);
                    Ok(WindResults::Hits(hits))
                }
                else if let WindResults::Hits(mut old_hits) = result
                {
                    let mut hits= vec!();
                    hits.append( &mut old_hits );
                    hits.append( &mut new_hits );
                    Ok(WindResults::Hits(hits))
                }
                else
                {
                    Err("when action is SearchHIts, expecting WindResult::Hits or WindResult::None".into())
                }
            }
            WindAction::Flags(flags) => {
                Ok(WindResults::None)
            }
        }
    }
}



#[derive(Clone,Serialize,Deserialize)]
pub enum WindResults
{
    None,
    Hits(Vec<WindHit>)
}

impl WindUp
{
    pub fn inc( &mut self, hop: StarKey, transaction: Id )
    {
        self.hops.push( hop );
        self.transactions.push(transaction);
    }

}

#[derive(Clone,Serialize,Deserialize)]
pub enum StarPattern
{
    Any,
    None,
    StarKey(StarKey),
    StarKind(StarKind)
}


impl StarPattern
{
    pub fn is_match( &self, info: &StarInfo ) -> bool
    {
        match self
        {
            StarPattern::Any => {true}
            StarPattern::None => {false}
            StarPattern::StarKey(star) => {
                *star == info.star
            }
            StarPattern::StarKind(kind) => {
                *kind == info.kind
            }
        }
    }


    pub fn is_single_match(&self) -> bool
    {
        match self
        {
            StarPattern::StarKey(_) => {true}
            StarPattern::StarKind(_) => {false}
            StarPattern::Any => {false}
            StarPattern::None => {false}
        }
    }
}

#[derive(Clone,Serialize,Deserialize)]
pub struct WindDown
{
    pub missed: Option<StarKey>,
    pub result: WindResults,
    pub wind_up: WindUp,
    pub transactions: Vec<Id>,
    pub hops : Vec<StarKey>
}

impl WindDown
{
    pub fn pop(&mut self)
    {
        self.transactions.pop();
        self.hops.pop();
    }
}

#[derive(Clone,Serialize,Deserialize,Hash,Eq,PartialEq)]
pub struct WindHit
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
   pub reply_to: Option<Id>
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
            reply_to: Option::None
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
            reply_to: Option::None
        }
    }


    pub fn reply(&self, payload: StarMessagePayload)->ProtoMessage
    {
        let mut proto = ProtoMessage::new();
        proto.to = Option::Some(self.from.clone());
        proto.reply_to = Option::Some(self.id.clone());
        proto.payload = payload;
        proto
    }

    pub fn resubmit(self, expect: MessageExpect, tx: broadcast::Sender<MessageUpdate>, rx: broadcast::Receiver<MessageUpdate> ) -> ProtoMessage
    {
        let mut proto = ProtoMessage::with_txrx(tx,rx);
        proto.to = Option::Some(self.from.clone());
        proto.expect = expect;
        proto.reply_to = Option::Some(self.id.clone());
        proto.payload = self.payload;
        proto
    }

}

#[derive(Clone,Serialize,Deserialize)]
pub enum StarMessagePayload
{
   None,
   Pledge,
   Space(SpaceMessage),
   Ok,
   Error(String),
   Ack(MessageAck),
   Sequence(SequenceMessage)
}

impl StarMessagePayload{
    pub fn is_ack(&self)->bool
    {
        match self{
            StarMessagePayload::Ack(_) => {
                true
            }
            _ => {
                false
            }
        }
    }
}

#[derive(Clone,Serialize,Deserialize)]
pub enum SequenceMessage
{
    Request,
    Response(u64)
}

#[derive(Clone,Serialize,Deserialize)]
pub struct MessageAck
{
    pub id: Id,
    pub kind: MessageAckKind
}

#[derive(Clone,Serialize,Deserialize)]
pub enum MessageAckKind
{
    Hop(StarKey),
    Received,
    Processing
}


#[derive(Clone,Serialize,Deserialize)]
pub struct SpaceMessage
{
    pub sub_space: SubSpaceKey,
    pub token: AuthToken,
    pub payload: SpacePayload
}

#[derive(Clone,Serialize,Deserialize)]
pub enum SpacePayload
{
    App(AppMessage),
    Request(RequestMessage),
    Report(ReportMessage),
    Assign(AssignMessage),
}

#[derive(Clone,Serialize,Deserialize)]
pub enum ReportMessage
{
   AppLocation(AppLocation)
}

#[derive(Clone,Serialize,Deserialize)]
pub enum AssignMessage
{
    App(AppAssign)
}

#[derive(Clone,Serialize,Deserialize)]
pub enum RequestMessage
{
    AppCreate(AppCreateRequest),
    AppSupervisor(AppSupervisorRequest),
    AppLookup(AppLookup),
    AppMessage(AppMessage),
    AppLabel(AppLabelRequest)
}

#[derive(Clone,Serialize,Deserialize)]
pub struct AppLabelRequest
{
    pub app: AppKey,
    pub labels: Labels
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
   None
}

#[derive(Clone,Serialize,Deserialize)]
pub enum ResponseMessage
{
    AppNotifyCreated(AppNotifyCreated),
    AppSupervisorReport(ApplicationSupervisorReport),
}



#[derive(Clone,Serialize,Deserialize)]
pub enum Event
{
    App(AppEvent),
    Actor(ActorEvent),
    Star(StarEvent)
}

#[derive(Clone,Serialize,Deserialize)]
pub enum ActorEvent
{
   StateChange(ActorState),
   Gathered(ActorGathered),
   Scattered(ActorScattered),
   Broadcast(ActorBroadcast),
   Destroyed
}

#[derive(Clone,Serialize,Deserialize)]
pub enum AppEvent
{
    Created,
    Ready,
    Destroyed
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
    Key(ActorKey)
}

impl ActorLookup
{
    pub fn app(&self) -> AppKey {
        match self
        {
            ActorLookup::Key(key) => {key.app.clone()}
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
    User(UserKey)
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
pub struct AppLookup
{
    pub name: String
}

#[derive(Clone,Serialize,Deserialize)]
pub struct AppCreateRequest
{
    pub labels: Labels,
    pub kind: AppKind,
    pub data: Arc<Vec<u8>>,
}

#[derive(Clone,Serialize,Deserialize)]
pub struct AppAssign
{
    pub app : AppInfo,
    pub data: Arc<Vec<u8>>,
}

#[derive(Clone,Serialize,Deserialize)]
pub struct AppNotifyCreated
{
    pub location: AppLocation
}

#[derive(Clone,Serialize,Deserialize)]
pub struct AppSupervisorRequest
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
            StarMessagePayload::None => "None".to_string(),
            StarMessagePayload::Pledge =>"Pledge".to_string(),
            StarMessagePayload::Space(_) => "Space".to_string(),
            StarMessagePayload::Ok => "Ok".to_string(),
            StarMessagePayload::Error(_) => "Error".to_string(),
            StarMessagePayload::Ack(_) => "Ack".to_string(),
            StarMessagePayload::Sequence(_) => "Sequence".to_string(),
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
            Frame::StarWind(wind)=>format!("StarWind({})",wind).to_string(),
            Frame::Watch(_) => format!("Watch").to_string(),
            Frame::Event(_) => format!("ActorEvent").to_string()
        };
        write!(f, "{}",r)
    }
}

impl fmt::Display for StarWind{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = match self {
            StarWind::Up(up) => format!("Up({})",&up.pattern).to_string(),
            StarWind::Down(_) => "Down".to_string()
        };
        write!(f, "{}",r)
    }
}

impl fmt::Display for StarPattern{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = match self {
            StarPattern::Any => "Any".to_string(),
            StarPattern::None => "None".to_string(),
            StarPattern::StarKey(key) => format!("{}",key).to_string(),
            StarPattern::StarKind(kind) => format!("{}",kind).to_string()
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
            ProtoFrame::Evolution(evol) => format!("Evolution({})", evol).to_string(),
            ProtoFrame::Sequence(seq) => format!("Sequence({})", seq).to_string(),
        };
        write!(f, "{}",r)
    }
}

impl fmt::Display for ProtoEvolution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = match self {
            ProtoEvolution::Request => "Request".to_string(),
            ProtoEvolution::Report => "Report".to_string()
        };
        write!(f, "{}", r)
    }
}

impl fmt::Display for ProtoSequence{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = match self {
            ProtoSequence::Request => "Request".to_string(),
            ProtoSequence::Reply(seq)=> format!("Reply({})",seq).to_string()
        };
        write!(f, "{}", r)
    }
}
