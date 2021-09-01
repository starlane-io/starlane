#[macro_use]
extern crate wasm_bindgen;

use wasm_membrane_guest::membrane::log;
use mechtron::{Mechtron, mechtron_register, Delivery};
use std::sync::Arc;




#[no_mangle]
pub extern "C" fn membrane_guest_init()
{
    log("Hello World! From: Wasm!");
    mechtron_register(Arc::new(Appy::new()));
}


pub struct Appy {

}

impl Appy {
    pub fn new()->Self{
        Self{}
    }
}

impl Mechtron for Appy  {
    fn name(&self) -> String {
        "appy".to_string()
    }

    fn deliver( &self, delivery: Delivery ) {
        log("Delivery of Message to Appy mechtron");
    }

}


