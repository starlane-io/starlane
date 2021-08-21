#[macro_use]
extern crate wasm_bindgen;

use wasm_membrane_guest::membrane::log;

#[no_mangle]
pub extern "C" fn mechtron_init()
{
    log("Hello World! From: Wasm!");
}
