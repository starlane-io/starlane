use crate::mesh::serde::portal::outlet;

pub mod serde;

use crate::mesh::serde::id;
use crate::mesh::serde::entity;
use mesh_portal_api;
use mesh_portal_serde::mesh::generic;
use crate::mesh::serde::id::Address;
use crate::mesh::serde::entity::request::{ReqEntity};
use crate::mesh::serde::messaging::{Exchange, ExchangeId};
use crate::mesh::serde::entity::response::RespEntity;
use ::serde::{Serialize,Deserialize};
use std::convert::TryInto;
use mesh_portal_serde::version::latest;
use crate::resource::{Kind, ResourceType};
use crate::error::Error;
use crate::mesh::serde::generic::payload::RcCommand;
use crate::mesh::serde::resource::command::common::StateSrc;

#[derive(Clone, Serialize, Deserialize)]
pub struct Request {
    pub id: String,
    pub to: Address,
    pub from: Address,
    pub entity: ReqEntity,
    pub exchange: Exchange,
}

impl Request {
    pub fn into_outlet_request(self) -> Result<latest::portal::outlet::Request,Error> {
       latest::portal::outlet::Request {
           from: self.from,
           entity: self.entity.convert()?,
           /*
           entity: &match self.entity {
               ReqEntity::Rc(rc) => {
                   latest::entity::request::ReqEntity::Rc(latest::entity::request::Rc {
                       command: match rc.command {
                          RcCommand::Create(create) => {
                            latest::payload::RcCommand::Create(latest::resource::command::create::Create{
                                template: create.template,
                                state: match create.state {
                                    StateSrc::Stateless => {latest::resource::command::common::StateSrc::Stateless}
                                    StateSrc::StatefulDirect(_) => {}
                                },
                                properties: Default::default(),
                                strategy: Strategy::Create,
                                registry: Default::default()
                            })
                          }
                       },
                       payload: rc.payload.convert()?
                   })
               }
               ReqEntity::Msg(msg) => {
                   latest::entity::request::ReqEntity::Msg(latest::entity::request::Msg{
                       action: msg.action,
                       path: msg.path,
                       payload: msg.payload.convert()?
                   })
               }
               ReqEntity::Http(http) => {
                   latest::entity::request::ReqEntity::Http(latest::entity::request::Http{
                       headers: http.headers,
                       method: http.method,
                       path: http.path,
                       body: http.body.convert()?
                   })
               }
           },

            */

           exchange: self.exchange
       }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Response{
    pub id: String,
    pub to: Address,
    pub from: Address,
    pub exchange: ExchangeId,
    pub entity: RespEntity
}




#[cfg(test)]
pub mod test {

    #[test]
    pub fn test() {

    }

}