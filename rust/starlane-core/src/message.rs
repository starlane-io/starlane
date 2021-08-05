pub mod resource;

use std::collections::HashSet;
use std::convert::Infallible;

use std::string::FromUtf8Error;

use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, oneshot};

use uuid::Uuid;

use starlane_resources::ResourceIdentifier;

use crate::error::Error;
use crate::frame::{
    Frame, MessageAck, MessagePayload, ReplyKind, SimpleReply, StarMessage, StarMessagePayload,
};

use crate::resource::{ResourceAddress, ResourceKind, ResourceType, Specific};
use crate::star::{StarCommand, StarKey};
use crate::star::shell::search::{StarSearchTransaction, TransactionResult};

pub type MessageId = Uuid;

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
pub enum Fail {
    Error(String),
    Reject(Reject),
    Unexpected {
        expected: String,
        received: String,
    },
    DoNotKnowSpecific(Specific),
    ResourceStateFinal(ResourceIdentifier),
    ResourceAddressAlreadyInUse(ResourceAddress),
    ResourceNotFound(ResourceIdentifier),
    WrongResourceType {
        expected: HashSet<ResourceType>,
        received: ResourceType,
    },
    WrongParentResourceType {
        expected: HashSet<ResourceType>,
        received: Option<ResourceType>,
    },
    ResourceTypeRequiresOwner,
    ChannelRecvErr,
    CannotSelectResourceHost,
    ResourceCannotGenerateAddress,
    SuitableHostNotAvailable(String),
    SqlError(String),
    CannotCreateNothingResourceTypeItIsThereAsAPlaceholderDummy,
    ResourceTypeMismatch(String),
    Timeout,
    InvalidResourceState(String),
    NoProvisioner(ResourceKind),
    QueueOverflow,
}

impl Fail {
    pub fn trace(fail: Fail) -> Self {
        error!("{}", fail.to_string().as_str());
        fail
    }

    pub fn expected(expected: &str) -> Self {
        error!(expected);
        Self::Unexpected {
            expected: expected.to_string(),
            received: "_".to_string(),
        }
    }

    pub fn unexpected<T: ToString>(expected: &str, received: T) -> Self {
        error!("expected: {}, received: {}", expected, received.to_string());
        Self::Unexpected {
            expected: expected.to_string(),
            received: received.to_string(),
        }
    }
}

impl ToString for Fail {
    fn to_string(&self) -> String {
        match self {
            Fail::Timeout => "Timeout".to_string(),
            Fail::Error(message) => format!("Error({})", message),
            Fail::Reject(_) => "Reject".to_string(),
            Fail::Unexpected { expected, received } => format!(
                "Unexpected( expected: {}, received: {} )",
                expected, received
            )
            .to_string(),
            Fail::DoNotKnowSpecific(_) => "DoNotKnowSpecific".to_string(),
            Fail::ResourceNotFound(id) => {
                format!("ResourceNotFound({})", id.to_string()).to_string()
            }
            Fail::WrongResourceType { expected, received } => format!(
                "WrongResourceType(expected:[{}],received:{})",
                hash_to_string(expected),
                received.to_string()
            ),
            Fail::ChannelRecvErr => "ChannelRecvErr".to_string(),
            Fail::ResourceTypeRequiresOwner => "ResourceTypeRequiresOwner".to_string(),
            Fail::CannotSelectResourceHost => "CannotSelectResourceHost".to_string(),
            Fail::WrongParentResourceType { expected, received } => format!(
                "WrongParentResourceType(expected:[{}],received:{})",
                hash_to_string(expected),
                match received {
                    None => "None".to_string(),
                    Some(expected) => expected.to_string(),
                }
            ),
            Fail::ResourceCannotGenerateAddress => "ResourceCannotGenerateAddress".to_string(),
            Fail::SuitableHostNotAvailable(detail) => {
                format!("SuitableHostNotAvailable({})", detail.to_string())
            }
            Fail::SqlError(detail) => format!("SqlError({})", detail.to_string()),
            Fail::CannotCreateNothingResourceTypeItIsThereAsAPlaceholderDummy => {
                "CannotCreateNothingResourceTypeItIsThereAsAPlaceholderDummy".to_string()
            }
            Fail::ResourceTypeMismatch(detail) => {
                format!("ResourceTypeMismatch({})", detail.to_string()).to_string()
            }
            Fail::ResourceStateFinal(_) => "ResourceStateFinal".to_string(),
            Fail::ResourceAddressAlreadyInUse(_) => "ResourceAddressAlreadyInUse".to_string(),
            Fail::InvalidResourceState(message) => {
                format!("InvalidResourceState({})", message).to_string()
            }
            Fail::NoProvisioner(kind) => format!("NoProvisioner({})", kind.to_string()).to_string(),
            Fail::QueueOverflow => "QueueOverflow".to_string(),
        }
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

impl From<tokio::sync::oneshot::error::RecvError> for Fail {
    fn from(_: tokio::sync::oneshot::error::RecvError) -> Self {
        Fail::ChannelRecvErr
    }
}

impl From<FromUtf8Error> for Fail {
    fn from(e: FromUtf8Error) -> Self {
        Fail::Error(e.to_string())
    }
}

impl From<yaml_rust::scanner::ScanError> for Fail {
    fn from(e: yaml_rust::scanner::ScanError) -> Self {
        Fail::Error(e.to_string())
    }
}

impl<T> From<tokio::sync::mpsc::error::SendError<T>> for Fail {
    fn from(e: tokio::sync::mpsc::error::SendError<T>) -> Self {
        Fail::Error(format!("{}", e.to_string()))
    }
}

impl From<actix_web::error::Canceled> for Fail {
    fn from(c: actix_web::error::Canceled) -> Self {
        Fail::Error(c.to_string())
    }
}

impl From<clap::Error> for Fail {
    fn from(e: clap::Error) -> Self {
        Fail::Error(e.to_string())
    }
}

impl From<strum::ParseError> for Fail {
    fn from(e: strum::ParseError) -> Self {
        Fail::Error(e.to_string())
    }
}

impl From<kube::Error> for Fail {
    fn from(e: kube::Error) -> Self {
        Fail::Error(e.to_string())
    }
}

impl From<starlane_resources::error::Error> for Fail {
    fn from(e: starlane_resources::error::Error) -> Self {
        Fail::Error(e.to_string())
    }
}

impl<T> From<tokio::sync::mpsc::error::TrySendError<T>> for Fail {
    fn from(_: tokio::sync::mpsc::error::TrySendError<T>) -> Self {
        Fail::QueueOverflow
    }
}

impl From<Infallible> for Fail {
    fn from(e: Infallible) -> Self {
        Fail::Error(format!("{}", e.to_string()))
    }
}

fn hash_to_string(hash: &HashSet<ResourceType>) -> String {
    let mut rtn = String::new();
    for i in hash.iter() {
        rtn.push_str(i.to_string().as_str());
        rtn.push_str(", ");
    }
    rtn
}
