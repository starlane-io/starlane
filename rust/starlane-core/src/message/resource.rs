use core::cell::Cell;
use core::option::Option;
use core::result::Result;
use core::result::Result::{Err, Ok};
use std::collections::HashSet;
use std::iter::FromIterator;

use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

use starlane_resources::ResourceIdentifier;

use crate::error::Error;
use crate::frame::{MessagePayload, Reply, SimpleReply, StarMessage, StarMessagePayload};

use crate::data::{BinSrc, DataSet};
use crate::message::{Fail, MessageId, ProtoStarMessage};
use crate::resource::{
    RemoteDataSrc, ResourceCreate, ResourceId, ResourceRecord, ResourceSelector, ResourceType,
};
use crate::star::{StarCommand, StarSkel};
use crate::util;
use tokio::time::Duration;

pub type MessageTo = ResourceIdentifier;

pub fn reverse(to: MessageTo) -> MessageFrom {
    MessageFrom::Resource(to)
}

#[derive(Clone, Serialize, Deserialize)]
pub enum MessageFrom {
    Inject,
    Resource(ResourceIdentifier),
}

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

impl ProtoMessage<ResourceRequestMessage> {
    pub async fn to_proto_star_message(mut self) -> Result<ProtoStarMessage, Error> {
        self.validate()?;
        let message = self.create()?;
        let mut proto = ProtoStarMessage::new();
        proto.to = message.to.clone().into();
        proto.trace = message.trace;
        proto.log = message.log;
        proto.payload = StarMessagePayload::MessagePayload(MessagePayload::Request(message));
        Ok(proto)
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

#[derive(Clone)]
pub struct Delivery<M>
where
    M: Clone,
{
    skel: StarSkel,
    star_message: StarMessage,
    pub payload: M,
}

impl<M> Delivery<M>
where
    M: Clone + Send + Sync + 'static,
{
    pub fn new(payload: M, star_message: StarMessage, skel: StarSkel) -> Self {
        Delivery {
            payload,
            star_message: star_message,
            skel: skel,
        }
    }
}

impl<M> Delivery<M>
where
    M: Clone + Send + Sync + 'static,
{
    pub fn result(&self, result: Result<Reply, Fail>) {
        match result {
            Ok(reply) => {
                self.reply(reply);
            }
            Err(fail) => {
                self.fail(fail);
            }
        }
    }

    pub fn result_ok<T>(&self, result: Result<T, Fail>) {
        match result {
            Ok(_) => {
                self.reply(Reply::Empty);
            }
            Err(fail) => {
                self.fail(fail);
            }
        }
    }

    pub fn result_rx<T>(self, mut rx: oneshot::Receiver<Result<T, Fail>>)
    where
        T: Send + Sync + 'static,
    {
        tokio::spawn(async move {
            match tokio::time::timeout(Duration::from_secs(15), rx).await {
                Ok(Ok(Ok(_))) => {
                    self.reply(Reply::Empty);
                }
                Ok(Ok(Err(fail))) => {
                    self.fail(fail);
                }
                Ok(Err(_)) => {
                    self.fail(Fail::Timeout);
                }
                Err(_) => {
                    self.fail(Fail::ChannelRecvErr);
                }
            }
        });
    }

    pub fn ok(&self) {
        self.reply(Reply::Empty);
    }

    pub fn reply(&self, reply: Reply) {
        let proto = self
            .star_message
            .reply(StarMessagePayload::Reply(SimpleReply::Ok(reply)));
        self.skel.messaging_api.send(proto);
    }

    pub fn fail(&self, fail: Fail) {
        let proto = self
            .star_message
            .reply(StarMessagePayload::Reply(SimpleReply::Fail(fail)));
        self.skel.messaging_api.send(proto);
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub enum ResourceRequestMessage {
    Create(ResourceCreate),
    Select(ResourceSelector),
    Unique(ResourceType),
    State,
}

/*
#[derive(Clone, Serialize, Deserialize)]
pub enum Reply {
    Empty,
    Key(ResourceKey),
    Address(ResourceAddress),
    Records(Vec<ResourceRecord>),
    Record(ResourceRecord),
    Message(MessageReply<ResourceResponseMessage>),
    Id(ResourceId),
    Seq(u64),
}
 */

#[derive(Clone, Serialize, Deserialize)]
pub enum ResourceResponseMessage {
    Resource(Option<ResourceRecord>),
    Resources(Vec<ResourceRecord>),
    Unique(ResourceId),
    State(DataSet<BinSrc>),
    Fail(Fail),
}

#[derive(Clone, Serialize, Deserialize)]
pub enum ActorMessage {}

pub type Raw = Vec<u8>;
pub type RawPayload = Vec<u8>;
pub type RawState = Vec<u8>;

impl Into<ProtoStarMessage> for Message<ResourceRequestMessage> {
    fn into(self) -> ProtoStarMessage {
        let mut proto = ProtoStarMessage::new();
        proto.to = self.to.clone().into();
        proto.payload = StarMessagePayload::MessagePayload(MessagePayload::Request(self));
        proto
    }
}

impl Into<ProtoStarMessage> for MessageReply<ResourceResponseMessage> {
    fn into(self) -> ProtoStarMessage {
        let mut proto = ProtoStarMessage::new();
        proto.payload = StarMessagePayload::MessagePayload(MessagePayload::Response(self));
        proto
    }
}

impl Into<StarMessagePayload> for Message<ResourceRequestMessage> {
    fn into(self) -> StarMessagePayload {
        StarMessagePayload::MessagePayload(MessagePayload::Request(self))
    }
}

impl Into<StarMessagePayload> for MessageReply<ResourceResponseMessage> {
    fn into(self) -> StarMessagePayload {
        StarMessagePayload::MessagePayload(MessagePayload::Response(self))
    }
}
