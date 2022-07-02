use mesh_portal::version::latest::bin::Bin;
use mesh_portal::version::latest::id::Point;
use mesh_portal::version::latest::messaging::{ReqShell, RespShell};
use mesh_portal::version::latest::particle::Stub;
use mesh_portal::version::latest::payload::Substance;
use mesh_portal::version::latest::selector::PointKindHierarchy;
use std::collections::HashSet;
use std::convert::{Infallible, TryFrom, TryInto};
use std::string::FromUtf8Error;

use cosmic_api::version::v0_0_1::id::id::{BaseKind, ToPoint};
use cosmic_api::version::v0_0_1::id::StarKey;
use cosmic_api::version::v0_0_1::sys::ParticleRecord;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, mpsc, oneshot};
use uuid::Uuid;

use crate::error::Error;
use crate::frame::{MessageAck, SimpleReply, StarMessage, StarMessagePayload};
use crate::star::shell::search::{StarSearchTransaction, TransactionResult};
use crate::star::surface::SurfaceApi;
use crate::star::StarCommand;
use crate::starlane::StarlaneCommand;

pub mod delivery;

pub type MessageId = String;

#[derive(Clone)]
pub enum ProtoStarMessageTo {
    None,
    Star(StarKey),
    Point(Point),
}

impl ToString for ProtoStarMessageTo {
    fn to_string(&self) -> String {
        match self {
            ProtoStarMessageTo::None => "None".to_string(),
            ProtoStarMessageTo::Star(star) => star.to_string(),
            ProtoStarMessageTo::Point(address) => address.to_string(),
        }
    }
}

impl ProtoStarMessageTo {
    pub fn is_none(&self) -> bool {
        match self {
            ProtoStarMessageTo::None => true,
            ProtoStarMessageTo::Star(_) => false,
            ProtoStarMessageTo::Point(_) => false,
        }
    }
}

impl From<StarKey> for ProtoStarMessageTo {
    fn from(key: StarKey) -> Self {
        ProtoStarMessageTo::Star(key)
    }
}

impl From<Point> for ProtoStarMessageTo {
    fn from(id: Point) -> Self {
        ProtoStarMessageTo::Point(id)
    }
}

impl From<Option<Point>> for ProtoStarMessageTo {
    fn from(id: Option<Point>) -> Self {
        match id {
            None => ProtoStarMessageTo::None,
            Some(id) => ProtoStarMessageTo::Point(id.into()),
        }
    }
}

pub struct ProtoStarMessage {
    pub to: ProtoStarMessageTo,
    pub payload: StarMessagePayload,
    pub tx: broadcast::Sender<MessageUpdate>,
    pub rx: broadcast::Receiver<MessageUpdate>,
    pub reply_to: Option<MessageId>,
    pub trace: bool,
    pub log: bool,
}

impl ProtoStarMessage {
    pub fn new() -> Self {
        let (tx, rx) = broadcast::channel(8);
        ProtoStarMessage::with_txrx(tx, rx)
    }

    pub fn with_txrx(
        tx: broadcast::Sender<MessageUpdate>,
        rx: broadcast::Receiver<MessageUpdate>,
    ) -> Self {
        ProtoStarMessage {
            to: ProtoStarMessageTo::None,
            payload: StarMessagePayload::None,
            tx: tx,
            rx: rx,
            reply_to: Option::None,
            trace: false,
            log: false,
        }
    }

    pub fn to(&mut self, to: ProtoStarMessageTo) {
        self.to = to;
    }

    pub fn reply_to(&mut self, reply_to: MessageId) {
        self.reply_to = Option::Some(reply_to);
    }

    pub fn validate(&self) -> Result<(), Error> {
        let mut errors = vec![];
        if self.to.is_none() {
            errors.push("must specify 'to' field");
        }
        if let StarMessagePayload::None = self.payload {
            errors.push("must specify a message payload");
        }

        if !errors.is_empty() {
            let mut rtn = String::new();
            for err in errors {
                rtn.push_str(err);
                rtn.push('\n');
            }
            return Err(rtn.into());
        }

        return Ok(());
    }
}

pub struct MessageReplyTracker {
    pub reply_to: MessageId,
    pub tx: broadcast::Sender<MessageUpdate>,
}

impl MessageReplyTracker {
    pub fn on_message(&self, message: &StarMessage) -> TrackerJob {
        match &message.payload {
            StarMessagePayload::Reply(reply) => match reply {
                SimpleReply::Ok(_reply) => {
                    self.tx.send(MessageUpdate::Result(MessageResult::Ok(
                        message.payload.clone(),
                    )));
                    TrackerJob::Done
                }
                SimpleReply::Fail(fail) => {
                    self.tx
                        .send(MessageUpdate::Result(MessageResult::Err(fail.to_string())));
                    TrackerJob::Done
                }
                SimpleReply::Ack(ack) => {
                    self.tx.send(MessageUpdate::Ack(ack.clone()));
                    TrackerJob::Continue
                }
            },
            _ => TrackerJob::Continue,
        }
    }
}

pub enum TrackerJob {
    Continue,
    Done,
}

#[derive(Clone)]
pub enum MessageUpdate {
    Ack(MessageAck),
    Result(MessageResult<StarMessagePayload>),
}

#[derive(Clone)]
pub enum MessageResult<OK> {
    Ok(OK),
    Err(String),
    Timeout,
}

impl<OK> ToString for MessageResult<OK> {
    fn to_string(&self) -> String {
        match self {
            MessageResult::Ok(_) => "Ok".to_string(),
            MessageResult::Err(err) => format!("Err({})", err),
            MessageResult::Timeout => "Timeout".to_string(),
        }
    }
}

#[derive(Clone)]
pub enum MessageExpect {
    None,
    Reply(ReplyKind),
}

#[derive(Clone)]
pub enum MessageExpectWait {
    Short,
    Med,
    Long,
}

impl MessageExpectWait {
    pub fn wait_seconds(&self) -> u64 {
        match self {
            MessageExpectWait::Short => 5,
            MessageExpectWait::Med => 10,
            MessageExpectWait::Long => 30,
        }
    }

    pub fn retries(&self) -> usize {
        match self {
            MessageExpectWait::Short => 5,
            MessageExpectWait::Med => 10,
            MessageExpectWait::Long => 15,
        }
    }
}

pub struct OkResultWaiter {
    rx: broadcast::Receiver<MessageUpdate>,
    tx: oneshot::Sender<StarMessagePayload>,
}

impl OkResultWaiter {
    pub fn new(
        rx: broadcast::Receiver<MessageUpdate>,
    ) -> (Self, oneshot::Receiver<StarMessagePayload>) {
        let (tx, osrx) = oneshot::channel();
        (OkResultWaiter { rx: rx, tx: tx }, osrx)
    }

    pub async fn wait(mut self) {
        tokio::spawn(async move {
            loop {
                if let Ok(MessageUpdate::Result(result)) = self.rx.recv().await {
                    match result {
                        MessageResult::Ok(payload) => {
                            self.tx.send(payload);
                        }
                        x => {
                            eprintln!(
                                "not expecting this results for OkResultWaiter...{} ",
                                x.to_string()
                            );
                            self.tx.send(StarMessagePayload::None);
                        }
                    }
                    break;
                }
            }
        });
    }
}

pub struct ResultWaiter {
    rx: broadcast::Receiver<MessageUpdate>,
    tx: oneshot::Sender<MessageResult<StarMessagePayload>>,
}

impl ResultWaiter {
    pub fn new(
        rx: broadcast::Receiver<MessageUpdate>,
    ) -> (Self, oneshot::Receiver<MessageResult<StarMessagePayload>>) {
        let (tx, osrx) = oneshot::channel();
        (ResultWaiter { rx: rx, tx: tx }, osrx)
    }

    pub async fn wait(mut self) {
        tokio::spawn(async move {
            loop {
                if let Ok(MessageUpdate::Result(result)) = self.rx.recv().await {
                    self.tx.send(result);
                    break;
                }
            }
        });
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reject {
    pub reason: String,
    pub kind: RejectKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RejectKind {
    Error,
    Denied,
    BadRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize, strum_macros::Display, Eq, PartialEq)]
pub enum ReplyKind {
    Empty,
    Record,
    Records,
    Stubs,
    AddressTksPath,
    Payload,
}

#[derive(Debug, Clone, Serialize, Deserialize, strum_macros::Display)]
pub enum Reply {
    Empty,
    Record(ParticleRecord),
    Records(Vec<ParticleRecord>),
    Stubs(Vec<Stub>),
    AddressTksPath(PointKindHierarchy),
    Payload(Substance),
}

impl Reply {
    pub fn kind(&self) -> ReplyKind {
        match self {
            Reply::Empty => ReplyKind::Empty,
            Reply::Record(_) => ReplyKind::Record,
            Reply::Records(_) => ReplyKind::Records,
            Reply::Stubs(_) => ReplyKind::Stubs,
            Reply::AddressTksPath(_) => ReplyKind::AddressTksPath,
            Reply::Payload(_) => ReplyKind::Payload,
        }
    }
}

fn hash_to_string(hash: &HashSet<BaseKind>) -> String {
    let mut rtn = String::new();
    for i in hash.iter() {
        rtn.push_str(i.to_string().as_str());
        rtn.push_str(", ");
    }
    rtn
}

impl From<ReqShell> for ProtoStarMessage {
    fn from(request: ReqShell) -> Self {
        let mut proto = ProtoStarMessage::new();
        proto.to = request.to.clone().to_point().into();
        proto.payload = StarMessagePayload::Request(request.into());
        proto
    }
}

impl From<RespShell> for ProtoStarMessage {
    fn from(response: RespShell) -> Self {
        let mut proto = ProtoStarMessage::new();
        proto.payload = StarMessagePayload::Response(response.into());
        proto
    }
}

#[derive(Clone)]
pub struct StarlaneMessenger {
    tx: mpsc::Sender<StarlaneCommand>,
}

impl StarlaneMessenger {
    pub fn new(tx: mpsc::Sender<StarlaneCommand>) -> Self {
        Self { tx }
    }
}

#[async_trait]
impl Transmitter for StarlaneMessenger {
    async fn direct(
        &self,
        request: cosmic_api::version::v0_0_1::wave::Ping,
    ) -> cosmic_api::version::v0_0_1::wave::Pong {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(StarlaneCommand::Request {
                request: request.clone(),
                tx,
            })
            .await;
        match rx.await {
            Ok(response) => response,
            Err(err) => {
                error!("{}", err.to_string());
                request.status(503)
            }
        }
    }

    fn send_sync(
        &self,
        request: cosmic_api::version::v0_0_1::wave::Ping,
    ) -> cosmic_api::version::v0_0_1::wave::Pong {
        let starlane_tx = self.tx.clone();
        tokio::runtime::Handle::current().block_on(async move {
            let (tx, rx) = oneshot::channel();
            starlane_tx
                .send(StarlaneCommand::Request {
                    request: request.clone(),
                    tx,
                })
                .await;
            match rx.await {
                Ok(response) => response,
                Err(err) => {
                    error!("{}", err.to_string());
                    request.status(503)
                }
            }
        })
    }

    async fn route(&self, wave: Wave) {
        todo!()
    }
}
