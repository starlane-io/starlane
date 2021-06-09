use core::cell::Cell;
use core::option::Option;
use core::result::Result;
use core::result::Result::{Err, Ok};
use serde::{Deserialize, Serialize, Serializer};

use tokio::sync::{oneshot, mpsc};
use crate::keys::{UserKey, ResourceKey, ResourceId, MessageId};
use crate::message::{Fail, ProtoStarMessage};
use crate::error::Error;
use crate::resource::{ResourceType, ResourceCreate, ResourceSelector, ResourceRecord, ResourceIdentifier};
use std::iter::FromIterator;
use crate::id::Id;
use crate::star::{StarKey, StarCommand, StarSkel};
use crate::frame::{StarMessagePayload, MessagePayload};
use std::collections::HashSet;
use crate::logger::Log::ProtoStar;


#[derive(Clone,Serialize,Deserialize)]
pub enum MessageFrom
{
    Inject(StarKey),
    Resource(ResourceIdentifier)
}

pub type MessageTo = ResourceIdentifier;


impl MessageTo{
    pub fn reverse(&self) -> MessageFrom {
          MessageFrom::Resource(self.clone())
    }
}


pub struct ProtoMessage<P> {
    pub id: MessageId,
    pub from: Option<MessageFrom>,
    pub to: Option<MessageTo>,
    pub payload: Option<P>,
    pub reply_tx: Cell<Option<oneshot::Sender<Result<MessageReply<P>,Fail>>>>,
}

impl <P> ProtoMessage <P>{
    pub fn new()->Self{

        ProtoMessage {
            id: MessageId::new_v4(),
            from: Option::None,
            to: Option::None,
            payload: None,
            reply_tx: Cell::new(Option::None),
        }
    }

    pub fn validate(&self) -> Result<(),Error> {
        if self.to.is_none() {
            Err("to must be set".into())
        }
        else if self.from.is_none() {
            Err("to must be set".into())
        }
        else if let Option::None = self.payload{
            Err("message payload cannot be None".into())
        }
        else{
            Ok(())
        }
    }

    pub fn create(self) ->Result<Message<P>,Error>{
        if let &Option::None = &self.payload {
            return Err("ResourceMessagePayload cannot be None".into());
        }

        Ok(Message {
            id: self.id,
            from: self.from.ok_or("need to set 'from' in ProtoMessage")?,
            to: self.to.ok_or("need to set 'to' in ProtoMessage")?,
            payload: self.payload.ok_or("need to set a payload in ProtoMessage")?,
        })
    }

    pub fn to(&mut self, to: MessageTo) {
        self.to = Option::Some(to);
    }

    pub fn from(&mut self, from: MessageFrom) {
        self.from = Option::Some(from);
    }

    pub fn payload(&mut self, payload: P ) {
        self.payload = Option::Some(payload);
    }


    pub fn reply(&mut self) -> oneshot::Receiver<Result<MessageReply<P>,Fail>> {
        let (tx,rx) = oneshot::channel();
        self.reply_tx.replace(Option::Some(tx));
        rx
    }

    pub fn sender(&mut self) -> Option<tokio::sync::oneshot::Sender<Result<MessageReply<P>,Fail>>>{
        self.reply_tx.replace(Option::None)
    }
}
pub struct ProtoMessageReply<P> {
    pub id: MessageId,
    pub from: Option<MessageFrom>,
    pub payload: Option<P>,
    pub reply_to: Option<MessageId>
}

impl <P> ProtoMessageReply <P>{
    pub fn new()->Self{

        ProtoMessageReply {
            id: MessageId::new_v4(),
            from: Option::None,
            payload: None,
            reply_to: Option::None
        }
    }

    pub fn validate(&self) -> Result<(),Error> {
        if self.reply_to.is_none() {
            Err("reply_to must be set".into())
        }
        else if self.from.is_none() {
            Err("from must be set".into())
        }
        else if let Option::None = self.payload{
            Err("message payload cannot be None".into())
        }
        else{
            Ok(())
        }
    }

    pub fn create(self) ->Result<MessageReply<P>,Error>{
        if let &Option::None = &self.payload {
            return Err("ResourceMessagePayload cannot be None".into());
        }

        Ok(MessageReply {
            id: self.id,
            from: self.from.ok_or("need to set 'from' in ProtoMessageReply")?,
            reply_to: self.reply_to.ok_or("need to set 'reply_to' in ProtoMessageReply")?,
            payload: self.payload.ok_or("need to set a payload in ProtoMessageReply")?,
        })
    }

    pub fn from(&mut self, from: MessageFrom) {
        self.from = Option::Some(from);
    }

    pub fn payload(&mut self, payload: P ) {
        self.payload = Option::Some(payload);
    }
}

#[derive(Clone,Serialize,Deserialize)]
pub struct Message<P>
{
    pub id: MessageId,
    pub from: MessageFrom,
    pub to: MessageTo,
    pub payload: P,
}

#[derive(Clone,Serialize,Deserialize)]
pub struct MessageReply<P>
{
    pub id: MessageId,
    pub from: MessageFrom,
    pub reply_to: MessageId,
    pub payload: P,
}

impl <P> Message<P>
{
    pub fn verify_type(&self, resource_type: ResourceType )->Result<(),Fail>
    {
        if self.to.resource_type() == resource_type {
            Ok(())
        } else {
            Err(Fail::WrongResourceType{
                received: resource_type,
                expected: HashSet::from_iter(vec![self.to.resource_type().clone()])
            })
        }
    }
}

pub struct Delivery<P>
{
    skel: StarSkel,
    from_star: StarKey,
    pub message: Message<P>
}

impl <P> Delivery<P>{
   pub async fn reply(&self, response: ResourceResponseMessage) -> Result<(),Error> {
      let mut proto = ProtoMessageReply::new();
      proto.payload = Option::Some(response);
      proto.from(MessageFrom::Resource(self.message.to.clone()));
      proto.reply_to = Option::Some(self.message.id.clone());
      let mut proto:ProtoStarMessage = proto.create()?.into();
      proto.to = self.from_star.clone().into();

      self.skel.star_tx.send( StarCommand::SendProtoMessage(proto)).await;

      Ok(())
   }
}

#[derive(Clone,Serialize,Deserialize)]
pub enum ResourceRequestMessage
{
    Create(ResourceCreate),
    Select(ResourceSelector),
    Unique(ResourceType)
}

#[derive(Clone,Serialize,Deserialize)]
pub enum ResourceResponseMessage
{
    Resource(Option<ResourceRecord>),
    Resources(Vec<ResourceRecord>),
    Unique(ResourceId)
}

#[derive(Clone,Serialize,Deserialize)]
pub enum ActorMessage{

}

pub type Raw=Vec<u8>;
pub type RawPayload=Vec<u8>;
pub type RawState=Vec<u8>;


impl Into<ProtoStarMessage> for Message<ResourceRequestMessage> {
    fn into(self) -> ProtoStarMessage {
        let mut proto = ProtoStarMessage::new();
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


