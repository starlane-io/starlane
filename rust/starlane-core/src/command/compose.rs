use mesh_portal_serde::version::latest::entity::request::create::{Create, CreateOp, Require};
use mesh_portal_serde::version::latest::entity::request::select::Select;
use mesh_portal_versions::version::v0_0_1::entity::request::create::Fulfillment;
use crate::error::Error;

pub enum CommandOp {
    Create(Create),
    Select(Select),
    Publish(CreateOp)
}

impl CommandOp {


}

pub enum Command {
    Create(Create),
    Select(Select)
}

