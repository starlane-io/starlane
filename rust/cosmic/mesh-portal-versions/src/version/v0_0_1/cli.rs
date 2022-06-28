use core::str::FromStr;
use nom::combinator::all_consuming;

use crate::error::MsgErr;
use crate::version::v0_0_1::bin::Bin;
use crate::version::v0_0_1::command::request::create::{Create, CreateCtx, CreateVar, Strategy};
use crate::version::v0_0_1::command::request::select::{Select, SelectCtx, SelectVar};
use crate::version::v0_0_1::command::request::get::{Get, GetCtx, GetVar};
use crate::version::v0_0_1::command::request::set::{Set, SetCtx, SetVar};
use crate::version::v0_0_1::parse::{command_line, Env};
use crate::version::v0_0_1::parse::error::result;
use crate::version::v0_0_1::substance::substance::Substance;
use crate::version::v0_0_1::util::ToResolved;
use serde::{Deserialize, Serialize};
use cosmic_nom::{new_span, Trace};

#[derive(Debug, Clone, Serialize, Deserialize )]
pub struct CommandTemplate {
    pub line: String,
    pub transfers: Vec<Trace>
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct RawCommand {
   pub line: String,
   pub transfers: Vec<Transfer>
}


#[derive(Debug,Clone,Serialize,Deserialize,Eq,PartialEq)]
pub struct Transfer {
    pub id: String,
    pub content: Bin
}

impl Transfer {
    pub fn new<N:ToString>( id: N, content: Bin ) -> Self {
        Self {
            id: id.to_string(),
            content
        }
    }
}