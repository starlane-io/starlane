#![allow(warnings)]

use nom::character::complete::multispace0;
use nom::combinator::recognize;
use nom::error::{ErrorKind, ParseError};
use nom::sequence::delimited;
use nom::{
    AsBytes, AsChar, Compare, CompareResult, FindSubstring, IResult, InputIter, InputLength,
    InputTake, InputTakeAtPosition, Needed, Offset, Slice,
};
use nom_locate::LocatedSpan;
use nom_supreme::error::{ErrorTree, GenericErrorTree};
use serde::{Deserialize, Serialize};

