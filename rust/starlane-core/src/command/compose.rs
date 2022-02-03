use std::str::FromStr;
use mesh_portal_serde::version::latest::entity::request::create::{Create, CreateOp, Require};
use mesh_portal_serde::version::latest::entity::request::select::Select;
use mesh_portal_versions::version::v0_0_1::entity::request::create::Fulfillment;
use nom::combinator::all_consuming;
use crate::command::parse::command_line;
use crate::error::Error;

pub enum CommandOp {
    Create(Create),
    Select(Select),
    Publish(CreateOp)
}

impl FromStr for CommandOp  {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(all_consuming(command_line)(s)?.1)
    }
}

impl CommandOp {

    pub fn requires(&self) -> Vec<Require> {
        match self {
            CommandOp::Create(_) => {vec![]}
            CommandOp::Select(_) => {vec![]}
            CommandOp::Publish(publish) => {
                publish.requirements.clone()
            }
        }
    }

}

pub enum Command {
    Create(Create),
    Select(Select)
}

