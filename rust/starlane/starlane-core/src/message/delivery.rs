use core::cell::Cell;
use core::option::Option;
use core::result::Result;
use core::result::Result::{Err, Ok};
use std::collections::HashSet;
use std::iter::FromIterator;

use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;
use tokio::time::Duration;

use crate::error::Error;
use crate::message::ProtoStarMessage;
use crate::star::{StarCommand, StarSkel};
use crate::util;
use crate::fail::Fail;
use crate::frame::{SimpleReply, StarMessage, StarMessagePayload};
use mesh_portal::version::latest::util::uuid;
use crate::message::Reply;
use std::ops::Deref;
use http::StatusCode;
use mesh_portal::error::MsgErr;
use mesh_portal::version::latest::entity::response::RespCore;
use mesh_portal::version::latest::id::Point;
use mesh_portal::version::latest::messaging::{ReqShell, RespShell};
use mesh_portal::version::latest::payload::{Call, CallKind, Errors, MsgCall, Substance};
use mesh_portal_versions::version::v0_0_1::id::id::ToPoint;
use mesh_portal_versions::version::v0_0_1::wave::Method;
use mesh_portal_versions::version::v0_0_1::parse::model::Subst;
use mesh_portal_versions::version::v0_0_1::substance::substance::HttpCall;
use mesh_portal_versions::version::v0_0_1::sys::ParticleRecord;

pub struct Delivery<M>
where
    M: Clone,
{
    skel: StarSkel,
    star_message: StarMessage,
    pub item: M,
    responded: bool
}

impl<M> Delivery<M>
where
    M: Clone + Send + Sync + 'static,
{
    pub fn new(item: M, star_message: StarMessage, skel: StarSkel) -> Self {
        Delivery {
            item,
            star_message: star_message,
            skel: skel,
            responded: false
        }
    }

    pub fn to(&self) -> Result<Point,Error> {
        match &self.star_message.payload {
            StarMessagePayload::Request(request) => {
                Ok(request.to.clone().to_point())
            }
            _ => {
                Err("this type of Delivery does not support to() resolution".into())
            }
        }
    }
}

impl<M> Deref for Delivery<M> where
    M: Clone + Send + Sync + 'static{
    type Target = M;

    fn deref(&self) -> &Self::Target {
        &self.item
    }
}

impl<M> Into<StarMessage> for Delivery<M> where
    M: Clone + Send + Sync + 'static{
    fn into(self) -> StarMessage {
        self.star_message
    }
}
impl <M> Delivery<M> where M: Clone + Send + Sync + 'static
{
  pub fn get_request(&self) -> Result<ReqShell,Error> {
      match &self.star_message.payload {
          StarMessagePayload::Request(request) => {
              Ok(request.clone())
          }
          _ => {
              Err("not a request delivery".into())
          }
      }
  }
}

impl Delivery<ReqShell>
{
    pub fn result( self, result: Result<Substance,Error>) {
        match result {
            Ok(payload) => {
                let request = self.item.core.clone();
                self.respond(request.ok(payload));
            }
            Err(err) => {
                self.fail(err.to_string());
            }
        }
    }

    pub fn respond(mut self, core: RespCore) {
        let response = RespShell {
            id: uuid(),
            to: self.item.from,
            from: self.item.to,
            core,
            response_to: self.item.id
        };
        let proto = self
            .star_message
            .reply(StarMessagePayload::Response(response));

        self.skel.messaging_api.star_notify(proto);
        self.responded = true;
    }

   pub fn ok(self, payload: Substance) -> Result<(),Error> {
        let core = RespCore {
            headers: Default::default(),
            status: StatusCode::from_u16(200).unwrap(),
            body: payload
        };
        self.respond(core);
        Ok(())
    }

    pub fn fail(self, fail: String ) ->Result<(),Error> {

        let core = RespCore {
            headers: Default::default(),
            status: StatusCode::from_u16(500).unwrap(),
            body: Substance::Text(fail)
        };
        self.respond(core);
        Ok(())
    }

    pub fn not_found(self) ->Result<(),Error> {

        let request = self.get_request()?;
        let core = RespCore {
            headers: Default::default(),
            status: StatusCode::from_u16(404).unwrap(),
            body: Substance::Empty
        };
        self.respond(core);
        Ok(())
    }

    pub fn err(self, status: u16, message: &str ) ->Result<(),Error> {

        let request = self.get_request()?;

        let core = RespCore {
            headers: Default::default(),
            status: StatusCode::from_u16(status)?,
            body: Substance::Errors(Errors::default(message))
        };
        self.respond(core);

        Ok(())
    }

    pub fn to_call(&self) -> Result<Call,MsgErr> {
        let kind = match &self.item.core.method {
            Method::Cmd(_) => {
                unimplemented!()
            }
            Method::Http(method) => {
                CallKind::Http(HttpCall::new(method.clone(), Subst::new(self.item.core.uri.path())? ))
            }
            Method::Msg(method) => {
                CallKind::Msg(MsgCall::new(method.clone(), Subst::new(self.item.core.uri.path())? ))
            }
        };

       Ok(Call {
           point: self.item.to.clone().to_point(),
           kind: kind.clone()
       })
    }
}

/*
impl <M:Clone>Drop for Delivery<M> {
    fn drop(&mut self) {
        if !self.responded {
            let core = ResponseCore {
                headers: Default::default(),
                status: StatusCode::from_u16(500).unwrap(),
                body: Payload::Errors(Errors::default("message delivery dropped unexpectedly."))
            };
            let response = Response {
                id: unique_id(),
                to: self.item.from,
                from: self.item.to,
                core,
                response_to: self.item.id
            };
            let proto = self
                .star_message
                .reply(StarMessagePayload::Response(response));

            self.skel.messaging_api.star_notify(proto);
            self.responded = true;
        }
    }
}

 */

/*impl<M> Delivery<M>
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

    pub fn result_ok<T>(&self, result: Result<T, Error>) {
        match result {
            Ok(_) => {
                self.reply(Reply::Empty);
            }
            Err(fail) => {
                self.fail(fail.into());
            }
        }
    }

    pub fn result_rx<T>(self, mut rx: oneshot::Receiver<Result<T, Error>>)
    where
        T: Send + Sync + 'static,
    {
        tokio::spawn(async move {
            match tokio::time::timeout(Duration::from_secs(15), rx).await {
                Ok(Ok(Ok(_))) => {
                    self.reply(Reply::Empty);
                }
                Ok(Ok(Err(fail))) => {
                    self.fail(fail.into());
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

 */

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
pub enum ActorMessage {}

pub struct DeliverySelector{
    selections: Vec<DeliverySelection>
}

pub enum DeliverySelection{
 Any
}

impl DeliverySelector {
    pub fn any() ->Self {
        Self {
            selections: vec![DeliverySelection::Any]
        }
    }
}





