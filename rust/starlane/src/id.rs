use std::fmt;
use serde::{Serialize,Deserialize};
use std::sync::atomic::{AtomicU64, Ordering};
use crate::keys::ResourceKey;

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Copy, Clone, Serialize, Deserialize)]
pub struct Id {
    pub sequence: u64,
    pub index: u64,
}

impl Id {
    pub fn new(sequence: u64, index: u64) -> Self {
        Id {
            sequence: sequence,
            index: index
        }
    }
}

impl fmt::Display for Id{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}",&self.sequence.clone() ,&self.index )
    }
}

pub struct IdSeq {
    sequence: u64,
    index: AtomicU64,
}

impl IdSeq {
    pub fn new(sequence: u64) -> Self {

        IdSeq::with_seq_and_start_index(sequence, 0)
    }

    pub fn with_seq_and_start_index(seq_id: u64, start_index: u64) -> Self {
        IdSeq {
            sequence: seq_id,
            index: AtomicU64::new(start_index ),
        }
    }

    pub fn seq_id(&self) -> u64 {
        self.sequence.clone()
    }

    pub fn next(&self) -> Id {
        Id {
            sequence: self.sequence.clone(),
            index: self.index.fetch_add(1, Ordering::Relaxed),
        }
    }
}
