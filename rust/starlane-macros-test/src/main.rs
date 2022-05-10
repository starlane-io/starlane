#![feature(structural_match)]
#![feature(box_syntax)]
#![feature(derive_eq)]
#![feature(fmt_internals)]

use std::convert::TryFrom;
use std::convert::TryInto;
use std::str::FromStr;

use nom::error::{context, ErrorKind, ParseError, VerboseError};
use serde::de::Error as OtherError;
use serde::{Deserialize, Serialize};

use starlane_core::error::Error;

use starlane_core::star::StarKind;
use starlane_macros::resources;
use starlane_core::particle::ResourceAddressPart;

pub fn parse_address_part(string: &str) -> Result<(&str, Vec<ResourceAddressPart>), Error> {
    unimplemented!()
}


fn main() {
    println!("Hello, world!");
}

