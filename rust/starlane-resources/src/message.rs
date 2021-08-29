use std::collections::HashSet;
use std::convert::Infallible;
use std::iter::FromIterator;
use std::string::FromUtf8Error;

use serde::{Deserialize, Serialize};
use uuid::Uuid;


use crate::{ResourceAddress, ResourceCreate, ResourceIdentifier, ResourceKey, ResourceKind, ResourceSelector, ResourceStub, ResourceType, SkewerCase, Specific,ResourceId};
use crate::error::Error;
use crate::data::{DataSet, BinSrc, Meta};

pub struct ProtoMessage<P> {
    pub id: MessageId,
    pub from: Option<MessageFrom>,
    pub to: Option<MessageTo>,
    pub payload: Option<P>,
    pub trace: bool,
    pub log: bool,
}

impl<P> ProtoMessage<P> {
    pub fn new() -> Self {
        ProtoMessage {
            id: MessageId::new_v4(),
            from: Option::None,
            to: Option::None,
            payload: None,
            trace: false,
            log: false,
        }
    }

    pub fn validate(&self) -> Result<(), Error> {
        if self.to.is_none() {
            Err("ProtoMessage: RESOURCE to must be set".into())
        } else if self.from.is_none() {
            Err("ProtoMessage: from must be set".into())
        } else if let Option::None = self.payload {
            Err("ProtoMessage: message payload cannot be None".into())
        } else {
            Ok(())
        }
    }

    pub fn create(self) -> Result<Message<P>, Error> {
        if let &Option::None = &self.payload {
            return Err("ResourceMessagePayload cannot be None".into());
        }

        Ok(Message {
            id: self.id,
            from: self.from.ok_or("need to set 'from' in ProtoMessage")?,
            to: self.to.ok_or("need to set 'to' in ProtoMessage")?,
            payload: self
                .payload
                .ok_or("need to set a payload in ProtoMessage")?,
            trace: self.trace,
            log: self.log,
        })
    }

    pub fn to(&mut self, to: MessageTo) {
        self.to = Option::Some(to);
    }

    pub fn from(&mut self, from: MessageFrom) {
        self.from = Option::Some(from);
    }

    pub fn payload(&mut self, payload: P) {
        self.payload = Option::Some(payload);
    }
}



pub struct ProtoMessageReply<P> {
    pub id: MessageId,
    pub from: Option<MessageFrom>,
    pub payload: Option<P>,
    pub reply_to: Option<MessageId>,
    pub trace: bool,
    pub log: bool,
}

impl<P> ProtoMessageReply<P> {
    pub fn new() -> Self {
        ProtoMessageReply {
            id: MessageId::new_v4(),
            from: Option::None,
            payload: None,
            reply_to: Option::None,
            trace: false,
            log: false,
        }
    }

    pub fn validate(&self) -> Result<(), Error> {
        if self.reply_to.is_none() {
            Err("ProtoMessageReply:reply_to must be set".into())
        } else if self.from.is_none() {
            Err("ProtoMessageReply: from must be set".into())
        } else if let Option::None = self.payload {
            Err("ProtoMessageReply: message payload cannot be None".into())
        } else {
            Ok(())
        }
    }

    pub fn create(self) -> Result<MessageReply<P>, Error> {
        if let &Option::None = &self.payload {
            return Err("ResourceMessagePayload cannot be None".into());
        }

        Ok(MessageReply {
            id: self.id,
            from: self.from.ok_or("need to set 'from' in ProtoMessageReply")?,
            reply_to: self
                .reply_to
                .ok_or("need to set 'reply_to' in ProtoMessageReply")?,
            payload: self
                .payload
                .ok_or("need to set a payload in ProtoMessageReply")?,
            trace: self.trace,
            log: self.log,
        })
    }

    pub fn from(&mut self, from: MessageFrom) {
        self.from = Option::Some(from);
    }

    pub fn payload(&mut self, payload: P) {
        self.payload = Option::Some(payload);
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Message<P> {
    pub id: MessageId,
    pub from: MessageFrom,
    pub to: MessageTo,
    pub payload: P,
    pub trace: bool,
    pub log: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct MessageReply<P> {
    pub id: MessageId,
    pub from: MessageFrom,
    pub reply_to: MessageId,
    pub payload: P,
    pub trace: bool,
    pub log: bool,
}

impl<P> Message<P> {
    pub fn verify_type(&self, resource_type: ResourceType) -> Result<(), Fail> {
        if self.to.resource_type() == resource_type {
            Ok(())
        } else {
            Err(Fail::WrongResourceType {
                received: resource_type,
                expected: HashSet::from_iter(vec![self.to.resource_type().clone()]),
            })
        }
    }
}






pub type MessageTo = ResourceIdentifier;

#[derive(Clone, Serialize, Deserialize)]
pub enum MessageFrom {
    Inject,
    Resource(ResourceIdentifier),
}

#[derive(Clone, Serialize, Deserialize)]
pub enum ResourceRequestMessage {
    Create(ResourceCreate),
    Select(ResourceSelector),
    Unique(ResourceType),
    State,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum ResourceResponseMessage {
    Resource(Option<ResourceStub>),
    Resources(Vec<ResourceStub>),
    Unique(ResourceId),
    State(DataSet<BinSrc>),
    Fail(Fail),
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ResourcePortMessage{
    pub port: String,
    pub payload: DataSet<BinSrc>
}

pub type Raw = Vec<u8>;
pub type RawPayload = Vec<u8>;
pub type RawState = Vec<u8>;

pub enum PortIdentifier{
    Key(PortKey),
    Address(PortAddress)
}


pub type PortIndex = u16;

#[derive(Clone,Serialize,Deserialize,Hash,Eq,PartialEq)]
pub struct PortKey {
    pub resource: ResourceIdentifier,
    pub port: PortIndex
}

#[derive(Clone,Serialize,Deserialize,Hash,Eq,PartialEq)]
pub struct PortAddress{
    pub resource: ResourceIdentifier,
    pub port: SkewerCase
}

pub type MessageId = Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Fail {
    Error(String),
/*    Reject(Reject),*/
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
        eprintln!("{}", fail.to_string().as_str());
        fail
    }

    pub fn expected(expected: &str) -> Self {
        eprintln!("{}",expected);
        Self::Unexpected {
            expected: expected.to_string(),
            received: "_".to_string(),
        }
    }

    pub fn unexpected<T: ToString>(expected: &str, received: T) -> Self {
        eprintln!("expected: {}, received: {}", expected, received.to_string());
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
                "WrongResourceType(expected:[_],received:{})",
                received.to_string()
            ),
            Fail::ChannelRecvErr => "ChannelRecvErr".to_string(),
            Fail::ResourceTypeRequiresOwner => "ResourceTypeRequiresOwner".to_string(),
            Fail::CannotSelectResourceHost => "CannotSelectResourceHost".to_string(),
            Fail::WrongParentResourceType { expected, received } => format!(
                "WrongParentResourceType(expected:[_],received:{})",
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



impl From<FromUtf8Error> for Fail {
    fn from(e: FromUtf8Error) -> Self {
        Fail::Error(e.to_string())
    }
}

impl From<crate::error::Error> for Fail {
    fn from(e: crate::error::Error) -> Self {
        Fail::Error(e.to_string())
    }
}


impl From<Infallible> for Fail {
    fn from(e: Infallible) -> Self {
        Fail::Error(format!("{}", e.to_string()))
    }
}

