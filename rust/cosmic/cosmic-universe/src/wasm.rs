use serde::{Serialize,Deserialize};
use crate::loc;

#[derive(Debug,Clone,Serialize,Deserialize)]
pub struct Timestamp {
  millis: i64
}

impl Timestamp {
    pub fn timestamp_millis(&self)-> i64 {
        self.millis
    }

    pub fn new(millis: i64) -> Self {
        Self {
            millis
        }
    }
}



#[no_mangle]
extern "C" {
    pub fn cosmic_timestamp() -> Timestamp;
    pub fn cosmic_uuid() -> loc::Uuid;
}