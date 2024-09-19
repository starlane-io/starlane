use crate::hyper::lane::{HyperAuthenticator, HyperwayEndpointFactory};
use crate::hyper::space::err::HyperErr;
use chrono::Utc;
use starlane_space::loc::ToBaseKind;
use starlane_space::wasm::Timestamp;
use std::str::FromStr;
use uuid::Uuid;
use platform::StoreFactory;
use crate::store::FileStore;

pub mod err;
pub mod global;
pub mod layer;
pub mod machine;
pub mod mem;
pub mod reg;
pub mod star;
pub mod tests;
pub mod platform;
pub mod service;

#[no_mangle]
pub extern "C" fn starlane_uuid() -> String {
    Uuid::new_v4().to_string()
}

#[no_mangle]
pub extern "C" fn starlane_timestamp() -> Timestamp {
    Timestamp::new(Utc::now().timestamp_millis())
}

