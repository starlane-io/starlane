use std::fmt;
use std::fmt::{Debug, Formatter};

use semver::SemVerError;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::time::error::Elapsed;

use cosmic_universe::hyper::{Assign, ParticleRecord};
use cosmic_universe::loc::StarKey;
use mesh_portal::version::latest::id::Point;
use mesh_portal::version::latest::messaging::{ReqShell, RespShell};

use crate::error::Error;
use crate::fail::{Fail, StarlaneFailure};
use crate::id::Id;
use crate::logger::Flags;
use crate::message::{MessageExpect, MessageId, MessageUpdate, ProtoStarMessage, Reply};
use crate::message::delivery::ActorMessage;
use crate::star::{Star, StarCommand, StarInfo, StarKind, StarNotify};
use crate::watch::{Notification, Watch, WatchKey};

#[derive(Debug, Clone, Serialize, Deserialize, strum_macros::Display)]
pub enum Frame {
    Proto(ProtoFrame),
    Diagnose(Diagnose),
    SearchTraversal(SearchTraversal),
    StarMessage(StarMessage),
    Watch(WatchFrame),
    Close,
}

#[derive(Debug, Clone, Serialize, Deserialize, strum_macros::Display)]
pub enum WatchFrame {
    Watch(Watch),
    UnWatch(WatchKey),
    Notify(Notification),
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
}

/*
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchInfo {
    pub id: Id,
    pub actor: ActorKey,
}

 */

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

#[derive(Debug, Clone, Serialize, Deserialize, strum_macros::Display)]
pub enum StarPattern {
    Any,
    None,
    StarKey(StarKey),
    StarKind(StarKind),
}

impl StarPattern {
    pub fn info_match(&self, info: &StarInfo) -> bool {
        match self {
            StarPattern::Any => true,
            StarPattern::None => false,
            StarPattern::StarKey(_) => self.key_match(&info.key),
            StarPattern::StarKind(pattern) => *pattern == info.kind,
        }
    }

    pub fn key_match(&self, star: &StarKey) -> bool {
        match self {
            StarPattern::Any => true,
            StarPattern::None => false,
            StarPattern::StarKey(pattern) => *star == *pattern,
            StarPattern::StarKind(_) => false,
        }
    }

    pub fn is_single_match(&self) -> bool {
        match self {
            StarPattern::StarKey(_) => true,
            StarPattern::StarKind(_) => false,
            StarPattern::Any => false,
            StarPattern::None => false,
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
        proto.payload = StarMessagePayload::Reply(SimpleReply::Fail(Fail::Starlane(
            StarlaneFailure::Error(err),
        )));
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
    Request(ReqShell),
    Response(RespShell),
    ResourceRegistry(ResourceRegistryRequest),
    ResourceHost(ResourceHostAction),
    //    Space(SpaceMessage),
    Reply(SimpleReply),
}

impl Debug for StarMessagePayload {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.write_str(match self {
            StarMessagePayload::None => "None",
            StarMessagePayload::Request(_) => "MessagePayload",
            StarMessagePayload::ResourceRegistry(_) => "ResourceRegistry",
            StarMessagePayload::ResourceHost(_) => "ResourceHost",
            StarMessagePayload::Reply(_) => "Reply",
            StarMessagePayload::Response(_) => "Response",
        });
        Ok(())
    }
}

/*
#[derive(Clone, Serialize, Deserialize)]
pub enum MessagePayload {
    Request(Message<ResourceRequestMessage>),
    Response(MessageReply<ResourceResponseMessage>),
    PortRequest(Message<ResourcePortMessage>),
    HttpRequest(Message<HttpRequest>),
}

 */

#[derive(Clone, Serialize, Deserialize)]
pub enum ResourceHostAction {
    //IsHosting(Address),
    Assign(Assign),
    Init(Point),
    GetState(Point),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResourceRegistryRequest {
    Location(ParticleRecord),
    Find(Point),
}

impl ToString for ResourceRegistryRequest {
    fn to_string(&self) -> String {
        match self {
            ResourceRegistryRequest::Location(_) => "Location".to_string(),
            ResourceRegistryRequest::Find(_) => "Find".to_string(),
        }
    }
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

/*
#[derive(Clone, Serialize, Deserialize, strum_macros::Display)]
pub enum Reply {
    Empty,
    Key(Address),
    Address(ResourceAddress),
    Records(Vec<ResourceRecord>),
    Record(ResourceRecord),
    Message(MessageReply<ResourceResponseMessage>),
    Id(ResourceId),
    State(DataSet<BinSrc>),
    ResourceValues(ResourceValues<ResourceStub>),
    Seq(u64),
    Port(DataSet<BinSrc>),
    HttpResponse(HttpResponse)
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
    Port,
    ResourceValues,
    HttpResponse
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
            Reply::Port(_) => *self == Self::Port,
            Reply::ResourceValues(_) => *self == Self::ResourceValues,
            Reply::HttpResponse(_) => *self == Self::HttpResponse
        }
    }
}
 */

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

/*
#[derive(Clone, Serialize, Deserialize)]
pub struct SpaceMessage {
    pub user: UserKey,
    pub payload: SpacePayload,
}

impl SpaceMessage {
    pub fn with_payload(&self, payload: SpacePayload) -> Self {
        SpaceMessage {
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

 */

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
            StarMessagePayload::Reply(reply) => format!("Reply({})", reply.to_string()),
            StarMessagePayload::ResourceRegistry(_) => "ResourceManager".to_string(),
            StarMessagePayload::ResourceHost(_) => "ResourceHost".to_string(),
            StarMessagePayload::Request(_) => "Request".to_string(),
            StarMessagePayload::Response(_) => "Response".to_string(),
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
        };
        write!(f, "{}", r)
    }
}
