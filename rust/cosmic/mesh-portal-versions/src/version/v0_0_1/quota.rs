use crate::version::v0_0_1::wave::{ReqShell, WaitTime};

// measured in seconds
#[derive(Clone)]
pub struct Timeouts {
    pub high: u64,
    pub med: u64,
    pub low: u64
}

impl Timeouts{
    pub fn from<W:Into<WaitTime>>(&self, wait: W) -> u64 {
        match wait.into() {
            WaitTime::High => self.high,
            WaitTime::Med => self.med,
            WaitTime::Low => self.low
        }
    }
}


impl Default for Timeouts {
    fn default() -> Self {
        Self {
            high: 5*60, // 5 minutes
            med: 1*60,  // 1 minute
            low: 15     // 15 seconds
        }
    }
}


