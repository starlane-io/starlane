use core::str::FromStr;
use nom::combinator::all_consuming;

use crate::bin::Bin;
use crate::command::request::create::{Create, CreateCtx, CreateVar, Strategy};
use crate::command::request::get::{Get, GetCtx, GetVar};
use crate::command::request::select::{Select, SelectCtx, SelectVar};
use crate::command::request::set::{Set, SetCtx, SetVar};
use crate::error::MsgErr;
use crate::parse::error::result;
use crate::parse::{command_line, Env};
use crate::substance::substance::Substance;
use crate::util::ToResolved;
use cosmic_nom::{new_span, Trace};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandTemplate {
    pub line: String,
    pub transfers: Vec<Trace>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct RawCommand {
    pub line: String,
    pub transfers: Vec<Transfer>,
}

impl RawCommand {
    pub fn new(line: String) -> Self {
        Self {
            line,
            transfers: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Transfer {
    pub id: String,
    pub content: Bin,
}

impl Transfer {
    pub fn new<N: ToString>(id: N, content: Bin) -> Self {
        Self {
            id: id.to_string(),
            content,
        }
    }
}
