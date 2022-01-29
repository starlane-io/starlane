use crate::mesh::serde::portal::outlet;

pub mod serde;

use crate::mesh::serde::{id};
use crate::resource;
use crate::mesh::serde::entity;
use mesh_portal_api;
use mesh_portal_serde::mesh::generic;
use crate::mesh::serde::id::Address;
use crate::mesh::serde::entity::request::{ReqEntity};
use crate::mesh::serde::messaging::{Exchange, ExchangeId};
use crate::mesh::serde::entity::response::RespEntity;
use ::serde::{Serialize,Deserialize};
use std::convert::{TryInto, TryFrom};
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
    pub fn into_outlet_request(self) -> Result<latest::portal::outlet::Request,mesh_portal_serde::error::Error> where
           crate::mesh::serde::payload::Payload: TryInto<mesh_portal_serde::version::latest::payload::Payload,Error=mesh_portal_serde::error::Error>,
           crate::mesh::serde::id::ResourceType: TryInto<mesh_portal_serde::version::latest::id::ResourceType,Error=mesh_portal_serde::error::Error>,
           crate::mesh::serde::id::Kind: TryInto<mesh_portal_serde::version::latest::id::Kind,Error=mesh_portal_serde::error::Error>,
           mesh_portal_serde::version::v0_0_1::generic::payload::Payload<mesh_portal_serde::version::v0_0_1::generic::id::KindParts<std::string::String>>: TryFrom<mesh_portal_serde::version::v0_0_1::generic::payload::Payload<resource::Kind>>

    {
       Ok(latest::portal::outlet::Request {
           to: self.to,
           from: self.from,
           entity: self.entity.convert()?,
           exchange: self.exchange
       })
    }
}

impl TryFrom<generic::Request> for Request {

    type Error = Error;

    fn try_from(request: generic::Request) -> Result<Self, Self::Error> {
        Ok(Request {
            id: request.id,
            to: request.to,
            from: request.from,
            entity: request.entity.try_into()?,
            exchange: request.exchange
        })
    }
}

#[derive(Debug,Clone, Serialize, Deserialize)]
pub struct Response{
    pub id: String,
    pub to: Address,
    pub from: Address,
    pub exchange: ExchangeId,
    pub entity: RespEntity
}


impl TryFrom<generic::Response> for Response{

    type Error = Error;

    fn try_from(response: generic::Response) -> Result<Self, Self::Error> {
        Ok(Response{
            id: response.id,
            to: response.to,
            from: response.from,
            entity: response.entity.try_into()?,
            exchange: response.exchange
        })
    }
}

#[cfg(test)]
pub mod test {

    #[test]
    pub fn test() {

    }

}