use cosmic_universe::loc;
use cosmic_universe::wasm::Timestamp;
use crate::{mechtron_timestamp, mechtron_uuid};
use crate::membrane::membrane_consume_string;

#[no_mangle]
extern "C" fn cosmic_uuid() -> loc::Uuid {
    loc::Uuid::from_unwrap(membrane_consume_string(unsafe { mechtron_uuid() }).unwrap())
}

#[no_mangle]
extern "C" fn cosmic_timestamp() -> Timestamp {
    Timestamp::new(unsafe { mechtron_timestamp() })
}
