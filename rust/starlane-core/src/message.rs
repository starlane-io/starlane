use std::collections::HashSet;
use std::convert::{Infallible, TryFrom, TryInto};
use std::string::FromUtf8Error;

use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, oneshot};
use uuid::Uuid;

use starlane_resources::message::{Message, MessageId, MessageReply, ProtoMessage, ResourceRequestMessage, ResourceResponseMessage, ResourcePortMessage};
use starlane_resources::ResourceIdentifier;

use crate::error::Error;
use crate::frame::{
    Frame, MessageAck, MessagePayload, ReplyKind, SimpleReply, StarMessage, StarMessagePayload,
};
use crate::resource::{ResourceAddress, ResourceKind, ResourceType, Specific};
use crate::star::{StarCommand, StarKey};
use crate::star::shell::search::{StarSearchTransaction, TransactionResult};
use starlane_resources::http::HttpRequest;

pub mod resource;

#[derive(Clone)]
pub enum ProtoStarMessageTo {
    None,
    Star(StarKey),
    Resource(ResourceIdentifier),
}

impl ProtoStarMessageTo {
    pub fn is_none(&self) -> bool {
        match self {
            ProtoStarMessageTo::None => true,
            ProtoStarMessageTo::Star(_) => false,
            ProtoStarMessageTo::Resource(_) => false,
        }
    }
}

impl From<StarKey> for ProtoStarMessageTo {
    fn from(key: StarKey) -> Self {
        ProtoStarMessageTo::Star(key)
    }
}

impl From<ResourceIdentifier> for ProtoStarMessageTo {
    fn from(id: ResourceIdentifier) -> Self {
        ProtoStarMessageTo::Resource(id)
    }
}

impl From<Option<ResourceIdentifier>> for ProtoStarMessageTo {
    fn from(id: Option<ResourceIdentifier>) -> Self {
        match id {
            None => ProtoStarMessageTo::None,
            Some(id) => ProtoStarMessageTo::Resource(id.into()),
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
impl TryFrom<ProtoMessage<ResourceRequestMessage>> for ProtoStarMessage {

    type Error = Error;

    fn try_from(proto: ProtoMessage<ResourceRequestMessage>) -> Result<Self, Self::Error> {
        proto.validate()?;
        let message = proto.create()?;
        let mut proto = ProtoStarMessage::new();
        proto.to = message.to.clone().into();
        proto.trace = message.trace;
        proto.log = message.log;
        proto.payload = StarMessagePayload::MessagePayload(MessagePayload::Request(message));
        Ok(proto)
    }
}

impl TryFrom<ProtoMessage<HttpRequest>> for ProtoStarMessage {

    type Error = Error;

    fn try_from(proto: ProtoMessage<HttpRequest>) -> Result<Self, Self::Error> {
        proto.validate()?;
        let message = proto.create()?;
        let mut proto = ProtoStarMessage::new();
        proto.to = message.to.clone().into();
        proto.trace = message.trace;
        proto.log = message.log;
        proto.payload = StarMessagePayload::MessagePayload(MessagePayload::HttpRequest(message));
        Ok(proto)
    }
}

impl TryFrom<ProtoMessage<ResourcePortMessage>> for ProtoStarMessage {

    type Error = Error;

    fn try_from(proto: ProtoMessage<ResourcePortMessage>) -> Result<Self, Self::Error> {
        proto.validate()?;
        let message = proto.create()?;
        message.try_into()
    }
}

impl TryFrom<Message<ResourcePortMessage>> for ProtoStarMessage {

    type Error = Error;

    fn try_from(message: Message<ResourcePortMessage>) -> Result<Self, Self::Error> {
        let mut proto = ProtoStarMessage::new();
        proto.to = message.to.clone().into();
        proto.trace = message.trace;
        proto.log = message.log;
        proto.payload = StarMessagePayload::MessagePayload(MessagePayload::PortRequest(message));
        Ok(proto)
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

fn hash_to_string(hash: &HashSet<ResourceType>) -> String {
    let mut rtn = String::new();
    for i in hash.iter() {
        rtn.push_str(i.to_string().as_str());
        rtn.push_str(", ");
    }
    rtn
}

impl From<Message<ResourceRequestMessage>> for ProtoStarMessage {
    fn from(message: Message<ResourceRequestMessage>) -> Self {
        let mut proto = ProtoStarMessage::new();
        proto.to = message.to.clone().into();
        proto.payload = StarMessagePayload::MessagePayload(MessagePayload::Request(message));
        proto
    }
}

impl From<MessageReply<ResourceResponseMessage>> for ProtoStarMessage {

    fn from(reply: MessageReply<ResourceResponseMessage>) -> Self {
        let mut proto = ProtoStarMessage::new();
        proto.payload = StarMessagePayload::MessagePayload(MessagePayload::Response(reply));
        proto
    }
}

impl From<Message<ResourceRequestMessage>> for StarMessagePayload {
    fn from( message: Message<ResourceRequestMessage> ) -> Self {
        StarMessagePayload::MessagePayload(MessagePayload::Request(message))
    }
}

impl From<MessageReply<ResourceResponseMessage>> for StarMessagePayload {
    fn from( message: MessageReply<ResourceResponseMessage> ) -> Self {
        StarMessagePayload::MessagePayload(MessagePayload::Response(message))
    }
}