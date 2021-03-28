use std::fmt;
use serde::{Serialize,Deserialize};

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Copy, Clone, Serialize, Deserialize)]
pub struct Id {
    pub sequence: i64,
    pub index: i64,
}

impl Id {
    pub fn new(sequence: i64, index: i64) -> Self {
        Id {
            sequence: sequence,
            index: index
        }
    }
}
impl fmt::Display for Id{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({},{})",self.sequence,self.index)
    }
}