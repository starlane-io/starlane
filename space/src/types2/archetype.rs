use crate::err::ParseErrs0;
use crate::parse::util::{new_span, result, Span};
use crate::parse::Res;
use serde::Serialize;
use std::fmt::Display;
use std::hash::Hash;
use std::str::FromStr;

/// anything that can be parsed
pub(crate) trait Archetype: Eq + PartialEq + Hash + Clone + Display + Serialize
//+DeserializeOwned
where
    Self: Sized,
{
    fn parser<I>(input: I) -> Res<I, Self>
    where
        I: Span;
}
