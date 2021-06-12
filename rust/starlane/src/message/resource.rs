use core::cell::Cell;
use core::option::Option;
use core::result::Result;
use core::result::Result::{Err, Ok};
use serde::{Deserialize, Serialize, Serializer};

use tokio::sync::{oneshot, mpsc};
use crate::keys::{UserKey, ResourceKey, ResourceId, MessageId};
use crate::message::{Fail, ProtoStarMessage};
use crate::error::Error;
use crate::resource::{ResourceType, ResourceCreate, ResourceSelector, ResourceRecord, ResourceIdentifier, RemoteDataSrc};
use std::iter::FromIterator;
use crate::id::Id;
use crate::star::{StarKey, StarCommand, StarSkel};
use crate::frame::{StarMessagePayload, MessagePayload, StarMessage, SimpleReply, Reply};
use std::collections::HashSet;
use crate::logger::Log::ProtoStar;
use crate::util;




#[derive(Clone,Serialize,Deserialize)]
pub enum MessageFrom
{
    Inject,
    Resource(ResourceIdentifier)
}

pub type MessageTo = ResourceIdentifier;


impl MessageTo{
    pub fn reverse(&self) -> MessageFrom {
          MessageFrom::Resource(self.clone())
    }
}


pub struct ProtoMessage<P,R> {
    pub id: MessageId,
    pub from: Option<MessageFrom>,
    pub to: Option<MessageTo>,
    pub payload: Option<P>,
    pub reply_tx: Cell<Option<oneshot::Sender<Result<MessageReply<R>,Fail>>>>,
    pub trace: bool,
    pub log: bool
}

impl <P,R> ProtoMessage <P,R>{
    pub fn new()->Self{

        ProtoMessage {
            id: MessageId::new_v4(),
            from: Option::None,
            to: Option::None,
            payload: None,
            reply_tx: Cell::new(Option::None),
            trace: false,
            log: false
        }
    }

    pub fn validate(&self) -> Result<(),Error> {
        if self.to.is_none() {
            Err("RESOURCE to must be set".into())
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

    pub fn create(self) ->Result<Message<P>,Error>{
        if let &Option::None = &self.payload {
            return Err("ResourceMessagePayload cannot be None".into());
        }

        Ok(Message {
            id: self.id,
            from: self.from.ok_or("need to set 'from' in ProtoMessage")?,
            to: self.to.ok_or("need to set 'to' in ProtoMessage")?,
            payload: self.payload.ok_or("need to set a payload in ProtoMessage")?,
            trace: self.trace,
            log: self.log
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


    pub fn reply(&mut self) -> oneshot::Receiver<Result<MessageReply<R>,Fail>> {
        let (tx,rx) = oneshot::channel();
        self.reply_tx.replace(Option::Some(tx));
        rx
    }

    pub fn sender(&mut self) -> Option<tokio::sync::oneshot::Sender<Result<MessageReply<R>,Fail>>>{
        self.reply_tx.replace(Option::None)
    }
}

impl ProtoMessage<ResourceRequestMessage,ResourceResponseMessage> {

    pub async fn to_proto_star_message(mut self)->Result<ProtoStarMessage,Error> {
        self.validate()?;
        let tx = self.sender();
        let message = self.create()?;
        let mut proto = ProtoStarMessage::new();
        proto.to = message.to.clone().into();
        proto.trace = message.trace;
        proto.log = message.log;
        proto.payload = StarMessagePayload::MessagePayload(MessagePayload::Request(message));
        let reply = proto.get_ok_result().await;

        if let Option::Some(tx) = tx {
            tokio::spawn( async move {
                let result = util::wait_for_it_whatever(reply).await;
                if let Ok(StarMessagePayload::Reply(SimpleReply::Ok(Reply::Message(message)))) = result {
                    tx.send(Ok(message) );
                } else if let Err(error) = result {
                    tx.send(Err(Fail::Error(format!("message reply error: {}",error))));
                }
            });
        }

        Ok(proto)
    }

}


pub struct ProtoMessageReply<P> {
    pub id: MessageId,
    pub from: Option<MessageFrom>,
    pub payload: Option<P>,
    pub reply_to: Option<MessageId>,
    pub trace: bool,
    pub log: bool
}

impl <P> ProtoMessageReply <P>{
    pub fn new()->Self{

        ProtoMessageReply {
            id: MessageId::new_v4(),
            from: Option::None,
            payload: None,
            reply_to: Option::None,
            trace: false,
            log: false
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
            trace: self.trace,
            log: self.log
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
    pub trace: bool,
    pub log: bool
}

#[derive(Clone,Serialize,Deserialize)]
pub struct MessageReply<P>
{
    pub id: MessageId,
    pub from: MessageFrom,
    pub reply_to: MessageId,
    pub payload: P,
    pub trace: bool,
    pub log: bool,
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

#[derive(Clone)]
pub struct Delivery<M> where M: Clone
{
    skel: StarSkel,
    star_message: StarMessage,
    pub message: M
}

impl <M> Delivery<M>  where M: Clone{
    pub fn new( message: M, star_message: StarMessage, skel: StarSkel ) -> Self {
        Delivery{
            message: message,
            star_message: star_message,
            skel: skel,
        }
    }
}

impl <P> Delivery<Message<P>> where P: Clone {
   pub async fn reply(&self, response: ResourceResponseMessage) -> Result<(),Error> {
      let mut proto = ProtoMessageReply::new();
      proto.payload = Option::Some(response);
      proto.reply_to = Option::Some(self.message.id.clone());
      proto.from = Option::Some(MessageFrom::Resource(self.message.to.clone()));
      let proto = self.star_message.reply(StarMessagePayload::Reply(SimpleReply::Ok( Reply::Message(proto.create()? ))));
      self.skel.star_tx.send( StarCommand::SendProtoMessage(proto)).await;
      Ok(())
   }
}

#[derive(Clone,Serialize,Deserialize)]
pub enum ResourceRequestMessage
{
    Create(ResourceCreate),
    Select(ResourceSelector),
    Unique(ResourceType),
    State
}

#[derive(Clone,Serialize,Deserialize)]
pub enum ResourceResponseMessage
{
    Resource(Option<ResourceRecord>),
    Resources(Vec<ResourceRecord>),
    Unique(ResourceId),
    State(RemoteDataSrc),
    Fail(Fail)
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


