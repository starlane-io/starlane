use core::str::FromStr;
use nom::combinator::all_consuming;

use crate::substance::Bin;
use crate::command::request::create::{Create, CreateCtx, CreateVar, Strategy};
use crate::command::request::get::{Get, GetCtx, GetVar};
use crate::command::request::select::{Select, SelectCtx, SelectVar};
use crate::command::request::set::{Set, SetCtx, SetVar};
use crate::error::UniErr;
use crate::parse::error::result;
use crate::parse::{command_line, Env};
use crate::substance::Substance;
use crate::util::ToResolved;
use cosmic_nom::{new_span, Trace};
use serde::{Deserialize, Serialize};
