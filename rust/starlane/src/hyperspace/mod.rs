use chrono::Utc;
use starlane::space::loc::ToBaseKind;
use starlane::space::wasm::Timestamp;
use std::str::FromStr;
use uuid::Uuid;

pub mod err;
pub mod global;
pub mod layer;
pub mod machine;
pub mod mem;
pub mod platform;
pub mod reg;
pub mod service;
pub mod star;
pub mod tests;

#[no_mangle]
pub extern "C" fn starlane_uuid() -> String {
    Uuid::new_v4().to_string()
}

#[no_mangle]
pub extern "C" fn starlane_timestamp() -> Timestamp {
    Timestamp::new(Utc::now().timestamp_millis())
}
