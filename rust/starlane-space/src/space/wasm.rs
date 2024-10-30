use std::sync::Arc;
use crate::space::loc;
use serde::{Deserialize, Serialize};
use crate::space::log::{LogAppender, RootLogger, RootLoggerBuilder};

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Timestamp {
    pub millis: i64,
}

impl Timestamp {
    pub fn timestamp_millis(&self) -> i64 {
        self.millis
    }

    pub fn new(millis: i64) -> Self {
        Self { millis }
    }
}

#[no_mangle]
extern "C" {
    pub fn starlane_timestamp() -> Timestamp;
    pub fn starlane_uuid() -> loc::Uuid;
}

