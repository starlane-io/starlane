use std::convert::Infallible;
use std::fmt::{Display, Formatter};
use std::string::FromUtf8Error;

use nom::error::VerboseError;
use semver::{ReqParseError, SemVerError};
use std::num::ParseIntError;

pub type MsgErr = cosmic_api::error::MsgErr;
pub type StatusErr = dyn cosmic_api::error::StatusErr;

