use crate::mesh::serde::portal::outlet;

pub mod serde;

use crate::mesh::serde::id;
use crate::mesh::serde::entity;
use mesh_portal_api;

pub type Message=mesh_portal_api::message::generic::Message<entity::request::ReqEntity,id::Identifier>;

#[cfg(test)]
pub mod test {

    #[test]
    pub fn test() {

    }

}