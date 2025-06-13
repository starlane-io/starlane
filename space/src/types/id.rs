use crate::point::Point;
use crate::types::Exact;

/// a globally defined [Point] + [Exact]
pub type Id = IdGen<Point,Exact>;


/// a generic definition of a `complete` identifier providing
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IdGen<Key, Type> {
    key: Key,
    r#type: Type,
}