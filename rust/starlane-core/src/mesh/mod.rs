use crate::mesh::serde::portal::outlet;

pub mod serde;

use crate::mesh::serde::id;
use crate::mesh::serde::entity;
use mesh_portal_api;
use mesh_portal_serde::mesh::generic;

pub type Request = generic::Request<entity::request::ReqEntity>;
pub type Response = generic::Response;
pub type Message = mesh_portal_api::message::generic::Message<entity::request::ReqEntity>;


#[cfg(test)]
pub mod test {

    #[test]
    pub fn test() {

    }

}