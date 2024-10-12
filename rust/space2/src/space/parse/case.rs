use crate::space::parse::ctx::InputCtx;
use crate::space::parse::nom::err::ParseErr;
use crate::space::parse::nom::{Input, Res};
use crate::space::parse::util::recognize;
use ::nom::branch::alt;
use ::nom::bytes::complete::{is_a, tag};
use ::nom::character::complete::{alpha1, alphanumeric1};
use ::nom::combinator::peek;
use ::nom::error::ErrorKind;
use ::nom::multi::many0;
use ::nom::sequence::tuple;
use ::nom::{AsChar, InputTakeAtPosition};
use alloc::string::{String, ToString};
use core::fmt;
use core::fmt::{Display, Formatter};
use core::ops::Deref;
use core::str::FromStr;
use nom::character::complete::alphanumeric0;
use starlane_primitive_macros::Case;

#[derive(Case, Debug, Clone, Eq, PartialEq, Hash)]
pub struct SkewerCase(String);

#[derive(Case, Debug, Clone, Eq, PartialEq, Hash)]
pub struct VarCase(String);
#[derive(Case, Debug, Clone, Eq, PartialEq, Hash)]
pub struct DomainCase(String);

#[derive(Case, Debug, Clone, Eq, PartialEq, Hash)]
pub struct CamelCase(String);

pub fn skewer_case<I: Input>(input: I) -> Res<I, SkewerCase> {
    recognize(tuple((peek(alpha1), many0(alt((alphanumeric1, tag("-")))))))(input)
        .map(|(next, rtn)| (next, SkewerCase(rtn.to_string())))
}

pub fn var_case<I: Input>(input: I) -> Res<I, VarCase> {
    recognize(tuple((peek(alpha1), many0(alt((alphanumeric1, tag("_")))))))(input)
        .map(|(next, rtn)| (next, VarCase(rtn.to_string())))
}

pub fn domain_case<I: Input>(input: I) -> Res<I, DomainCase> {
    recognize(tuple((
        peek(alpha1),
        many0(alt((alphanumeric1, tag("-"), tag(".")))),
    )))(input)
    .map(|(next, rtn)| (next, DomainCase(rtn.to_string())))
}

pub fn lowercase_alphanumeric<I: Input>(input: I) -> Res<I, I> {
    recognize(tuple((lowercase1, alphanumeric0)))(input)
}

pub fn lowercase1<T: Input>(i: T) -> Res<T, T>
where
    T: InputTakeAtPosition + nom::InputLength,
    <T as InputTakeAtPosition>::Item: AsChar,
{
    i.split_at_position1_complete(
        |item| {
            let char_item = item.as_char();
            !(char_item.is_alpha() && char_item.is_lowercase())
        },
        ErrorKind::AlphaNumeric,
    )
}
