use crate::membrane::mechtron_consume_string;
use crate::{mechtron_timestamp, mechtron_uuid};
use starlane_space::loc;
use starlane_space::wasm::Timestamp;

#[no_mangle]
extern "C" fn starlane_uuid() -> loc::Uuid {
    loc::Uuid::from_unwrap(mechtron_consume_string(unsafe { mechtron_uuid() }).unwrap())
}

#[no_mangle]
extern "C" fn starlane_timestamp() -> Timestamp {
    Timestamp::new(unsafe { mechtron_timestamp() })
}
