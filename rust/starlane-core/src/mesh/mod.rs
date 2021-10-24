use crate::mesh::serde::portal::outlet;

mod serde;

// always use these Request / Response within the Mesh, only use other specifics for particular serialization use cases
pub type Request = mesh_portal_serde::mesh::generic::Request<serde::entity::request::ReqEntity>;
pub type Response = mesh_portal_serde::mesh::generic::Response;


