use crate::point::Point;
use crate::types::Exact;

/// a globally defined [Point] + [Exact]
pub type Ident = IdentGen<Point,Exact>;


/// a generic definition of a `complete` identifier providing
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IdentGen<Point, Type> {
    point: Point,
    r#type: Type,
}