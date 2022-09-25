use cosmic_universe::loc;
use cosmic_universe::wasm::Timestamp;
use wasm_membrane_guest::membrane::membrane_consume_string;
use crate::{mechtron_timestamp, mechtron_uuid};

#[no_mangle]
extern "C" fn cosmic_uuid() -> loc::Uuid {
    loc::Uuid::from_unwrap(membrane_consume_string(unsafe { mechtron_uuid() }).unwrap())
}

#[no_mangle]
extern "C" fn cosmic_timestamp() -> Timestamp {
    Timestamp::new(unsafe { mechtron_timestamp() })
}
