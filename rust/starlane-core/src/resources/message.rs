use std::collections::HashSet;
use std::convert::{Infallible, TryInto};
use std::iter::FromIterator;
use std::string::FromUtf8Error;
use mesh_portal_serde::version::latest::entity::request::ReqEntity;
use mesh_portal_serde::version::latest::entity::response::RespEntity;
use mesh_portal_serde::version::latest::id::Address;
use mesh_portal_serde::version::latest::messaging::{ExchangeId, ExchangeType, Request, Response};
use mesh_portal_serde::version::latest::payload::Payload;

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::error::Error;

use crate::message::{ProtoStarMessage, ProtoStarMessageTo};
use crate::frame::StarMessagePayload;

pub enum MessageFrom {
    Inject,
    Address(Address)
}

pub struct ProtoRequest {
    pub id: MessageId,
    pub from: Option<MessageFrom>,
    pub to: Option<MessageTo>,
    pub entity: Option<ReqEntity>,
    pub exchange: ExchangeType,
    pub trace: bool,
    pub log: bool,
}

impl ProtoRequest {
    pub fn new() -> Self {
        ProtoRequest {
            id: MessageId::new_v4(),
            from: Option::None,
            to: Option::None,
            entity: None,
            trace: false,
            log: false,
            exchange: ExchangeType::Notification
        }
    }

    pub fn validate(&self) -> Result<(), Error> {
        if self.to.is_none() {
            Err("ProtoMessage: RESOURCE to must be set".into())
        } else if self.from.is_none() {
            Err("ProtoMessage: from must be set".into())
        } else if let Option::None = self.entity{
            Err("ProtoMessage: message entity cannot be None".into())
        } else {
            Ok(())
        }
    }

    pub fn create(self) -> Result<Request, Error> {
        if let &Option::None = &self.entity{
            return Err("Request entity cannot be None".into());
        }
        let from = match self.from.ok_or("need to set 'from' in ProtoRequest")? {
            MessageFrom::Inject => {
                Err("Inject must be replaced with a Star address before creating")
            }
            MessageFrom::Address(address) => {
                Ok(address)
            }
        }?;
        Ok(Request{
            id: self.id.to_string(),
            from,
            to: self.to.ok_or("need to set 'to' in ProtoRequest")?,
            entity: self
                .entity
                .ok_or("need to set an entity in ProtoRequest")?,
        })
    }

    pub fn to(&mut self, to: MessageTo) {
        self.to = Option::Some(to);
    }

    pub fn from(&mut self, from: MessageFrom) {
        self.from = Option::Some(from);
    }

    pub fn entity(&mut self, entity: ReqEntity) {
        self.entity = Option::Some(entity);
    }
}



pub struct ProtoResponse {
    pub id: MessageId,
    pub to: Address,
    pub from: Option<Address>,
    pub entity: Option<RespEntity>,
    pub exchange: Option<ExchangeId>,
    pub trace: bool,
    pub log: bool,
}

impl ProtoResponse {

    pub fn validate(&self) -> Result<(), Error> {
        if self.exchange.is_none() {
            Err("ProtoMessageReply:exchangemust be set".into())
        } else if self.from.is_none() {
            Err("ProtoMessageReply: from must be set".into())
        } else if let Option::None = self.entity {
            Err("ProtoMessageReply: message entity cannot be None".into())
        } else {
            Ok(())
        }
    }

    pub fn create(self) -> Result<Response, Error> {
        unimplemented!();

/*        Ok(Response{
            id: self.id.to_string(),
            to: self.to,
            from: self.from.ok_or("need to set 'from' in ProtoMessageReply")?,
            exchange: self
                .exchange
                .ok_or("need to set 'exchange' in ProtoMessageReply")?,
            entity: self.
                entity
                .ok_or("need to set an entity in ProtoMessageReply")?,
        })

 */
    }

    pub fn from(&mut self, from: Address ) {
        self.from = Option::Some(from);
    }

    pub fn payload(&mut self, payload: Payload) {
        self.entity = Option::Some(RespEntity::Ok(payload));
    }
}


pub type MessageTo = Address;


pub type MessageId = Uuid;



