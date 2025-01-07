use crate::point::Point;
use crate::types::ExtType;

/// a globally defined [Point] + [ExtType]
pub type Id = IdGen<Point, ExtType>;


/// a generic definition of a `complete` identifier providing
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IdGen<Key, Type> {
    key: Key,
    r#type: Type,
}