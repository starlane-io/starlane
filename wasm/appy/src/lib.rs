#[macro_use]
extern crate wasm_bindgen;

use wasm_membrane_guest::membrane::log;
use mechtron::Mechtron;

#[no_mangle]
pub extern "C" fn mechtron_init()
{
    log("Hello World! From: Wasm!");
}


pub struct Appy {

}

impl Mechtron for Appy  {
    fn name(&self) -> String {
        "appy".to_string()
    }


}

