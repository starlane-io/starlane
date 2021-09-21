use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use crate::error::Error;

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Copy, Clone, Serialize, Deserialize)]
pub struct Id {
    pub sequence: u64,
    pub index: u64,
}

pub type SequenceId = u64;
pub type IndexId = u64;

impl Id {
    pub fn new(sequence: u64, index: u64) -> Self {
        Id {
            sequence: sequence,
            index: index,
        }
    }
}

impl ToString for Id {
    fn to_string(&self) -> String {
        format!("{}_{}", self.sequence, self.index)
    }
}

impl FromStr for Id {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut split = s.split("_");
        let sequence = SequenceId::from_str(split.next().ok_or("expected sequence before '-'")?)?;
        let index = IndexId::from_str(split.next().ok_or("expected index after '-'")?)?;
        Ok(Id {
            sequence: sequence,
            index: index,
        })
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
            index: AtomicU64::new(start_index),
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
