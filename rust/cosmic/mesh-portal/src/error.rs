use std::convert::Infallible;
use std::fmt::{Display, Formatter};
use std::string::FromUtf8Error;

use nom::error::VerboseError;
use semver::{ReqParseError, SemVerError};
use std::num::ParseIntError;

pub type MsgErr = mesh_portal_versions::error::MsgErr;
pub type StatusErr = dyn mesh_portal_versions::error::StatusErr;

