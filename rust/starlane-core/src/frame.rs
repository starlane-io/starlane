use std::fmt;
use std::fmt::{Debug, Formatter};

use semver::SemVerError;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::time::error::Elapsed;

use starlane_resources::{AssignResourceStateSrc, ResourceAssign, ResourceCreate, ResourceIdentifier, ResourceSelector, ResourceStatus, ResourceStub, ResourceAddress, Labels};
use starlane_resources::data::{BinSrc, DataSet};
use starlane_resources::message::{Fail, Message, MessageId, MessageReply, RawState, ResourceRequestMessage, ResourceResponseMessage};

use crate::error::Error;
use crate::id::Id;
use crate::logger::Flags;
use crate::message::{MessageExpect, MessageUpdate, ProtoStarMessage};
use crate::message::resource::ActorMessage;
use crate::star::{Star, StarCommand, StarInfo, StarKey, StarKind, StarNotify, StarSubGraphKey};
use crate::watch::{Notification, Watch, WatchKey};
use crate::resource::{ResourceId, ResourceRegistration, ResourceRecord, ResourceType, ResourceKey, ResourceSliceStatus, SubSpaceKey, UserKey, AppKey, ActorKey};

#[derive(Debug, Clone, Serialize, Deserialize,strum_macros::Display)]
pub enum Frame {
    Proto(ProtoFrame),
    Diagnose(Diagnose),
    SearchTraversal(SearchTraversal),
    StarMessage(StarMessage),
    Watch(WatchFrame),
    Close,
}


#[derive(Debug, Clone, Serialize, Deserialize,strum_macros::Display)]
pub enum WatchFrame {
    Watch(Watch),
    UnWatch(WatchKey),
    Notify(Notification)
}



#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SearchTraversal {
    Up(SearchWindUp),
    Down(SearchWindDown),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProtoFrame {
    StarLaneProtocolVersion(i32),
    ReportStarKey(StarKey),
    GatewaySelect,
    GatewayAssign(Vec<StarSubGraphKey>),
}



#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchInfo {
    pub id: Id,
    pub actor: ActorKey,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StarMessageAck {
    pub from: StarKey,
    pub to: StarKey,
    pub id: Id,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Diagnose {
    Ping,
    Pong,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchWindUp {
    pub from: StarKey,
    pub pattern: StarPattern,
    pub hops: Vec<StarKey>,
    pub transactions: Vec<u64>,
    pub max_hops: usize,
    pub action: TraversalAction,
}

impl SearchWindUp {
    pub fn new(from: StarKey, pattern: StarPattern, action: TraversalAction) -> Self {
        SearchWindUp {
            from: from,
            pattern: pattern,
            action: action,
            hops: vec![],
            transactions: vec![],
            max_hops: 255,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TraversalAction {
    SearchHits,
    Flags(Flags),
}

impl TraversalAction {
    pub fn update(
        &self,
        mut new_hits: Vec<SearchHit>,
        result: SearchResults,
    ) -> Result<SearchResults, Error> {
        match self {
            TraversalAction::SearchHits => {
                if let SearchResults::None = result {
                    let mut hits = vec![];
                    hits.append(&mut new_hits);
                    Ok(SearchResults::Hits(hits))
                } else if let SearchResults::Hits(mut old_hits) = result {
                    let mut hits = vec![];
                    hits.append(&mut old_hits);
                    hits.append(&mut new_hits);
                    Ok(SearchResults::Hits(hits))
                } else {
                    Err(
                        "when action is SearchHIts, expecting WindResult::Hits or WindResult::None"
                            .into(),
                    )
                }
            }
            TraversalAction::Flags(_flags) => Ok(SearchResults::None),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SearchResults {
    None,
    Hits(Vec<SearchHit>),
}

impl SearchWindUp {
    pub fn inc(&mut self, hop: StarKey, transaction: u64) {
        self.hops.push(hop);
        self.transactions.push(transaction);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize,strum_macros::Display)]
pub enum StarPattern {
    Any,
    None,
    StarKey(StarKey),
    StarKind(StarKind),
    StarKeySubgraph(Vec<StarSubGraphKey>)
}

impl StarPattern {
    pub fn info_match(&self, info: &StarInfo) -> bool {
        match self {
            StarPattern::Any => true,
            StarPattern::None => false,
            StarPattern::StarKey(_) => {
                self.key_match(&info.key)
            }
            StarPattern::StarKind(pattern) => *pattern == info.kind,
            StarPattern::StarKeySubgraph(_) => {
                self.key_match(&info.key)
            }
        }
    }

    pub fn key_match(&self, star: &StarKey) -> bool {
        match self {
            StarPattern::Any => true,
            StarPattern::None => false,
            StarPattern::StarKey(pattern) => *star == *pattern,
            StarPattern::StarKind(_) => false,
            StarPattern::StarKeySubgraph(pattern) => {
                // TODO match tail end of subgraph
                *pattern == star.subgraph
            }
        }
    }


    pub fn is_single_match(&self) -> bool {
        match self {
            StarPattern::StarKey(_) => true,
            StarPattern::StarKind(_) => false,
            StarPattern::Any => false,
            StarPattern::None => false,
            StarPattern::StarKeySubgraph(_) => false
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchWindDown {
    pub missed: Option<StarKey>,
    pub result: SearchResults,
    pub wind_up: SearchWindUp,
    pub transactions: Vec<u64>,
    pub hops: Vec<StarKey>,
}

impl SearchWindDown {
    pub fn pop(&mut self) {
        self.transactions.pop();
        self.hops.pop();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub struct SearchHit {
    pub star: StarKey,
    pub hops: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StarMessage {
    pub from: StarKey,
    pub to: StarKey,
    pub id: MessageId,
    pub payload: StarMessagePayload,
    pub reply_to: Option<MessageId>,
    pub trace: bool,
    pub log: bool,
}

impl StarMessage {
    pub fn new(id: MessageId, from: StarKey, to: StarKey, payload: StarMessagePayload) -> Self {
        StarMessage {
            id: id,
            from: from,
            to: to,
            payload: payload,
            reply_to: Option::None,
            trace: false,
            log: true,
        }
    }

    pub fn to_central(id: MessageId, from: StarKey, payload: StarMessagePayload) -> Self {
        StarMessage {
            id: id,
            from: from,
            to: StarKey::central(),
            payload: payload,
            reply_to: Option::None,
            trace: false,
            log: false,
        }
    }

    pub fn forward(&self, _to: &StarKey) -> ProtoStarMessage {
        let mut proto = ProtoStarMessage::new();
        proto.to = self.to.clone().into();
        proto.payload = self.payload.clone();
        proto
    }

    /*
    pub async fn reply_tx(
        self,
        star_tx: mpsc::Sender<StarCommand>,
    ) -> oneshot::Sender<StarMessagePayload> {
        let message = self;
        let (tx, rx) = oneshot::channel();
        tokio::spawn(async move {
            match rx.await {
                Ok(payload) => {
                    let proto = message.reply(payload);
                    star_tx.send(StarCommand::SendProtoMessage(proto));
                }
                Err(_error) => {
                    let proto = message.reply_err("no reply".to_string());
                    star_tx.send(StarCommand::SendProtoMessage(proto));
                }
            }
        });

        tx
    }

     */

    pub fn fail(&self, fail: Fail) -> ProtoStarMessage {
        self.reply(StarMessagePayload::Reply(SimpleReply::Fail(fail)))
    }

    pub fn ok(&self, reply: Reply) -> ProtoStarMessage {
        self.reply(StarMessagePayload::Reply(SimpleReply::Ok(reply)))
    }

    pub fn reply(&self, payload: StarMessagePayload) -> ProtoStarMessage {
        let mut proto = ProtoStarMessage::new();
        proto.to = self.from.clone().into();
        proto.reply_to = Option::Some(self.id.clone());
        proto.payload = payload;
        proto
    }

    pub fn reply_err(&self, err: String) -> ProtoStarMessage {
        let mut proto = ProtoStarMessage::new();
        proto.to = self.from.clone().into();
        proto.reply_to = Option::Some(self.id.clone());
        proto.payload = StarMessagePayload::Reply(SimpleReply::Fail(Fail::Error(err)));
        proto
    }

    pub fn reply_ok(&self, reply: Reply) -> ProtoStarMessage {
        let mut proto = ProtoStarMessage::new();
        proto.to = self.from.clone().into();
        proto.reply_to = Option::Some(self.id.clone());
        proto.payload = StarMessagePayload::Reply(SimpleReply::Ok(reply));
        proto
    }

    pub fn resubmit(
        self,
        tx: broadcast::Sender<MessageUpdate>,
        rx: broadcast::Receiver<MessageUpdate>,
    ) -> ProtoStarMessage {
        let mut proto = ProtoStarMessage::with_txrx(tx, rx);
        proto.to = self.from.clone().into();
        proto.reply_to = Option::Some(self.id.clone());
        proto.payload = self.payload;
        proto
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub enum StarMessagePayload {
    None,
    MessagePayload(MessagePayload),
    ResourceManager(RegistryAction),
    ResourceHost(ResourceHostAction),
    Space(SpaceMessage),
    Reply(SimpleReply),
    UniqueId(ResourceId),
}

impl Debug for StarMessagePayload {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.write_str(match self {
            StarMessagePayload::None => "None",
            StarMessagePayload::MessagePayload(_) => "MessagePayload",
            StarMessagePayload::ResourceManager(_) => "ResourceManager",
            StarMessagePayload::ResourceHost(_) => "ResourceHost",
            StarMessagePayload::Space(_) => "Space",
            StarMessagePayload::Reply(_) => "Reply",
            StarMessagePayload::UniqueId(_) => "UniqueId",
        });
        Ok(())
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub enum MessagePayload {
    Request(Message<ResourceRequestMessage>),
    Response(MessageReply<ResourceResponseMessage>),
    Actor(Message<ActorMessage>),
}

#[derive(Clone, Serialize, Deserialize)]
pub enum ResourceHostAction {
    //IsHosting(ResourceKey),
    Assign(ResourceAssign<AssignResourceStateSrc<DataSet<BinSrc>>>),
    Init(ResourceKey),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RegistryAction {
    Register(ResourceRegistration),
    Location(ResourceRecord),
    Find(ResourceIdentifier),
    Status(ResourceStatusReport),
    UniqueResourceId {
        parent: ResourceIdentifier,
        child_type: ResourceType,
    },
}

impl ToString for RegistryAction {
    fn to_string(&self) -> String {
        match self {
            RegistryAction::Register(_) => "Register".to_string(),
            RegistryAction::Location(_) => "Location".to_string(),
            RegistryAction::Find(_) => "Find".to_string(),
            RegistryAction::Status(_) => "Status".to_string(),
            RegistryAction::UniqueResourceId { .. } => "UniqueResourceId".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceStatusReport {
    pub key: ResourceKey,
    pub status: ResourceStatus,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ResourceSliceStatusReport {
    pub key: ResourceKey,
    pub status: ResourceSliceStatus,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum SimpleReply {
    Ok(Reply),
    Fail(Fail),
    Ack(MessageAck),
}

impl ToString for SimpleReply {
    fn to_string(&self) -> String {
        match self {
            SimpleReply::Ok(ok) => format!("Ok({})", ok.to_string()),
            SimpleReply::Fail(fail) => format!("Fail({})", fail.to_string()),
            SimpleReply::Ack(_ack) => "Ack".to_string(),
        }
    }
}

impl StarMessagePayload {
    pub fn is_ack(&self) -> bool {
        match self {
            StarMessagePayload::Reply(reply) => match reply {
                SimpleReply::Ack(_) => true,
                _ => false,
            },
            _ => false,
        }
    }
}

#[derive(Clone, Serialize, Deserialize, strum_macros::Display)]
pub enum Reply {
    Empty,
    Key(ResourceKey),
    Address(ResourceAddress),
    Records(Vec<ResourceRecord>),
    Record(ResourceRecord),
    Message(MessageReply<ResourceResponseMessage>),
    Id(ResourceId),
    State(DataSet<BinSrc>),
    Seq(u64),
}

#[derive(Clone, Eq, PartialEq, strum_macros::Display)]
pub enum ReplyKind {
    Empty,
    Key,
    Address,
    Records,
    Record,
    Message,
    Id,
    Seq,
    State,
}

impl ReplyKind {
    pub fn is_match(&self, reply: &Reply) -> bool {
        match reply {
            Reply::Empty => *self == Self::Empty,
            Reply::Key(_) => *self == Self::Key,
            Reply::Address(_) => *self == Self::Address,
            Reply::Records(_) => *self == Self::Records,
            Reply::Record(_) => *self == Self::Record,
            Reply::Message(_) => *self == Self::Message,
            Reply::Id(_) => *self == Self::Id,
            Reply::Seq(_) => *self == Self::Seq,
            Reply::State(_) => *self == Self::State,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub enum SequenceMessage {
    Request,
    Response(u64),
}

#[derive(Clone, Serialize, Deserialize)]
pub struct MessageAck {
    pub id: Id,
    pub kind: MessageAckKind,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum MessageAckKind {
    Hop(StarKey),
    Received,
    Processing,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SpaceMessage {
    pub sub_space: SubSpaceKey,
    pub user: UserKey,
    pub payload: SpacePayload,
}

impl SpaceMessage {
    pub fn with_payload(&self, payload: SpacePayload) -> Self {
        SpaceMessage {
            sub_space: self.sub_space.clone(),
            user: self.user.clone(),
            payload: payload,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub enum SpacePayload {
    Reply(SpaceReply),
    Server(ServerPayload),
    Supervisor(SupervisorPayload),
}

#[derive(Clone, Serialize, Deserialize)]
pub enum SupervisorPayload {
    AppSequenceRequest(AppKey),
}

#[derive(Clone, Serialize, Deserialize)]
pub enum ServerPayload {
    SequenceResponse(u64),
}

#[derive(Clone, Serialize, Deserialize)]
pub enum SpaceReply {
    AppSequenceResponse(u64),
}

#[derive(Clone, Serialize, Deserialize)]
pub enum AssignMessage {}

#[derive(Clone, Serialize, Deserialize)]
pub struct AppLabelRequest {
    pub app: AppKey,
    pub labels: Labels,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum Event {
    App(AppEvent),
    Actor(ActorEvent),
    Star(StarEvent),
}

#[derive(Clone, Serialize, Deserialize)]
pub enum ActorEvent {
    StateChange(RawState),
    Gathered(ActorGathered),
    Scattered(ActorScattered),
    Broadcast(ActorBroadcast),
    Destroyed,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum AppEvent {
    Created,
    Ready,
    Destroyed,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum StarEvent {
    Lane(LaneEvent),
}

#[derive(Clone, Serialize, Deserialize)]
pub struct LaneEvent {
    pub star: StarKey,
    pub kind: LaneEventKind,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum LaneEventKind {
    Connect,
    Disconnect,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ActorGathered {
    pub to: ResourceKey,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ActorScattered {
    pub from: ResourceKey,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ActorBroadcast {
    pub topic: String,
    pub data: Vec<u8>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ActorLocationRequest {
    pub lookup: ActorLookup,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ActorLocationReport {
    pub resource: ResourceKey,
    pub location: ResourceRecord,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum ActorLookup {
    Key(ActorKey),
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Rejection {
    pub message: String,
}

impl fmt::Display for Diagnose {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = match self {
            Diagnose::Ping => "Ping",
            Diagnose::Pong => "Pong",
        };
        write!(f, "{}", r)
    }
}

impl fmt::Display for StarMessagePayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = match self {
            StarMessagePayload::None => "None".to_string(),
            StarMessagePayload::Space(_) => "Space".to_string(),
            StarMessagePayload::Reply(reply) => format!("Reply({})", reply.to_string()),
            StarMessagePayload::ResourceManager(_) => "ResourceManager".to_string(),
            StarMessagePayload::ResourceHost(_) => "ResourceHost".to_string(),
            StarMessagePayload::UniqueId(_) => "UniqueId".to_string(),
            StarMessagePayload::MessagePayload(_) => "MessagePayload".to_string(),
        };
        write!(f, "{}", r)
    }
}


impl fmt::Display for SearchTraversal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = match self {
            SearchTraversal::Up(up) => format!("Up({})", &up.pattern.to_string()).to_string(),
            SearchTraversal::Down(_) => "Down".to_string(),
        };
        write!(f, "{}", r)
    }
}



impl fmt::Display for ProtoFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = match self {
            ProtoFrame::StarLaneProtocolVersion(version) => {
                format!("StarLaneProtocolVersion({})", version).to_string()
            }
            ProtoFrame::ReportStarKey(key) => {
                format!("ReportStarKey({})", key.to_string()).to_string()
            }
            ProtoFrame::GatewaySelect => format!("GatewaySelect").to_string(),
            ProtoFrame::GatewayAssign { .. } => "GatewayAssign".to_string(),
        };
        write!(f, "{}", r)
    }
}



