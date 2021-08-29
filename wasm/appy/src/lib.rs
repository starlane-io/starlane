#[macro_use]
extern crate wasm_bindgen;

use wasm_membrane_guest::membrane::log;

#[no_mangle]
pub extern "C" fn mechtron_init()
{
    log("Hello World! From: Wasm!");
}

#[no_mangle]
pub extern "C" fn appy_init()
{
    log("Hello World! This is APPY init!");
}


#[no_mangle]
pub extern "C" fn appy_web( )
{
    log("Hello World! This is APPY init!");
}

