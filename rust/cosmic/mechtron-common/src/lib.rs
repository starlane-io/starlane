#![allow(warnings)]
#![no_std]

extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

use serde::{Deserialize, Serialize};

#[cfg(test)]
pub mod test {

    #[test]
    pub fn test() {}
}
