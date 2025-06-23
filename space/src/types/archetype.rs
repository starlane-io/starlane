use serde::{Deserialize, Serialize};
use std::fmt::Display;
use std::hash::Hash;
use serde::de::DeserializeOwned;
use crate::parse::Res;
use crate::parse::util::Span;

/// anything that can be parsed
pub(crate) trait Archetype: Eq+PartialEq+Hash+Clone+Display+Serialize+DeserializeOwned
where
    Self: Sized,
{
    fn parser<I>(input: I) -> Res<I, Self>
    where
        I: Span;
}