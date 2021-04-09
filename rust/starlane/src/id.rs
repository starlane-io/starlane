use std::fmt;
use serde::{Serialize,Deserialize};
use std::sync::atomic::{AtomicI64, Ordering};

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

pub struct IdSeq {
    sequence: i64,
    index: AtomicI64,
}

impl IdSeq {
    pub fn new(sequence: i64) -> Self {

        IdSeq::with_seq_and_start_index(sequence, 0)
    }

    pub fn with_seq_and_start_index(seq_id: i64, start_index: i64) -> Self {
        IdSeq {
            sequence: seq_id,
            index: AtomicI64::new(start_index ),
        }
    }

    pub fn seq_id(&self) -> i64 {
        self.sequence
    }

    pub fn next(&self) -> Id {
        Id {
            sequence: self.sequence,
            index: self.index.fetch_add(1, Ordering::Relaxed),
        }
    }
}