use std::str::FromStr;
use mesh_portal::version::latest::entity::request::create::{Create, CreateOp, Require};
use mesh_portal::version::latest::entity::request::get::Get;
use mesh_portal::version::latest::entity::request::select::Select;
use mesh_portal::version::latest::entity::request::create::Fulfillment;
use mesh_portal::version::latest::entity::request::set::Set;
use mesh_portal_versions::version::v0_0_1::entity::request::create::CreateOpVar;
use mesh_portal_versions::version::v0_0_1::entity::request::get::GetVar;
use mesh_portal_versions::version::v0_0_1::entity::request::set::SetVar;
use mesh_portal_versions::version::v0_0_1::parse::ToResolved;
use mesh_portal_versions::version::v0_0_1::span::new_span;
use nom::combinator::all_consuming;
use crate::command::parse::command_line;
use crate::error::Error;

pub enum Strategy {
    Commit,
    Ensure
}



pub enum CommandOp {
    Create(Create),
    Select(Select),
    Publish(CreateOpVar),
    Set(SetVar),
    Get(GetVar)
}

impl CommandOp {
    pub fn set_strategy( &mut self, strategy: Strategy ) {
        match self {
            CommandOp::Create(create) => {
               match strategy {
                   Strategy::Commit => {
                       create.strategy = mesh_portal::version::latest::entity::request::create::Strategy::Create;
                   }
                   Strategy::Ensure => {
                       create.strategy = mesh_portal::version::latest::entity::request::create::Strategy::Ensure;
                   }
               }
            }
            CommandOp::Select(_) => {}
            CommandOp::Publish(_) => {}
            CommandOp::Set(_) => {}
            CommandOp::Get(_) => {}
        }
    }
}

impl FromStr for CommandOp  {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = new_span(s);
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
            CommandOp::Set(_) => {vec![]}
            CommandOp::Get(_) => {vec![]}
        }
    }

    pub fn to_command(self) -> Result<Command,Error>{
        if self.requires().is_empty() {
            match self {
                CommandOp::Create(create) => {
                    Ok(Command::Create(create))
                }
                CommandOp::Select(select) => {
                    Ok(Command::Select(select))
                }
                CommandOp::Set(set) => {
                    let set = set.collapse().unwrap();
                    Ok(Command::Set(set))
                }
                CommandOp::Get(get) => {
                    let get = get.collapse().unwrap();
                    Ok(Command::Get(get))
                }
                _ => {
                    Err("cannon converted a CommandOp to a Command if it has requirements.".into() )
                }
            }
        } else {
            Err("cannon converted a CommandOp to a Command if it has requirements.".into() )
        }
    }

}

pub enum Command {
    Create(Create),
    Select(Select),
    Set(Set),
    Get(Get)
}

