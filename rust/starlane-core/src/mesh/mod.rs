use crate::mesh::serde::portal::outlet;

pub mod serde;

use crate::mesh::serde::id;
use crate::mesh::serde::entity;
use mesh_portal_api;
use mesh_portal_serde::mesh::generic;
use crate::mesh::serde::id::Address;
use crate::mesh::serde::entity::request::ReqEntity;
use crate::mesh::serde::messaging::{Exchange, ExchangeId};
use crate::mesh::serde::entity::response::RespEntity;
use serde::{Serialize,Deserialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Request {
    pub id: String,
    pub to: Address,
    pub from: Address,
    pub entity: ReqEntity,
    pub exchange: Exchange,
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