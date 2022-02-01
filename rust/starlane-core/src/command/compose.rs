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
    pub fn requirements(&self) -> Vec<Require>{
        match self {
            CommandOp::Create(_) => {vec![]}
            CommandOp::Select(_) => {vec![]}
            CommandOp::Publish(op) => {op.requirements()}
        }
    }

    pub fn fulfill(self, mut fulfillment: Vec<Fulfillment> ) -> Result<Command,Error> {
        match self {
            CommandOp::Create(create) => {
                Ok(Command::Create(create))
            }
            CommandOp::Select(select) => {
                Ok(Command::Select(select))
            }
            CommandOp::Publish(op) => {
                if fulfillment.is_empty() {
                    Err("CreateOp fulfillment cannot be empty".into())
                } else if fulfillment.len() > 1 {
                    Err("CreateOp fulfillment should only have one".into())
                } else {
                    let fulfillment = fulfillment.remove(0);
                    let require = op.requirements.first().ok_or("expected CreateOp to have at least one requirement".into() )?;

                    if let Fulfillment::File {name, bin } = fulfillment {
                        if let Require::File(req_name) = require {
                            if name != req_name {
                                return Err("incorrect required name".into())
                            }
                            return Ok(Command::Create(op.fulfillment(bin)));
                        } else {
                            return Err("expected requirement to be a File".into())
                        }
                    }

                }
            }
        }
    }

}

pub enum Command {
    Create(Create),
    Select(Select)
}

