use alloc::string::ToString;
use core::fmt;
use core::fmt::Display;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Point();


impl Display for Point
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", "Point".to_string())
    }
}

impl Default for Point {
    fn default() -> Self {
        Point()
    }
}