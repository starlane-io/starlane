use std::str::FromStr;
use derive_name::Name;
use crate::parse::Res;
use crate::parse::util::Span;
use crate::point::Point;
use crate::types::{Abstract, Full};
use crate::types::private::{Delimited, Parsable};

/// a globally defined [Point] + [Full]
pub type Ident = IdentGen<Point, Full>;



/// a generic definition of a `complete` identifier providing
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IdentGen<Point, Type> {
    point: Point,
    r#type: Type,
}