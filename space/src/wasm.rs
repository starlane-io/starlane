use crate::log::LogAppender;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Timestamp {
    pub millis: u64,
}

impl Timestamp {
    pub fn timestamp_millis(&self) -> u64 {
        self.millis
    }

    pub fn new(millis: u64) -> Self {
        Self { millis }
    }
}

/*
#[no_mangle]
extern "C" {
    pub fn starlane_timestamp() -> Timestamp;
    pub fn starlane_uuid() -> loc::Uuid;
}

 */
