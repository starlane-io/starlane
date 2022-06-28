#![allow(warnings)]
#[no_std]


#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate strum_macros;



use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

pub mod version;
pub mod error;


#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
    }
}
