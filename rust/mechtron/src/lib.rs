use std::sync::{RwLock, Arc};
use std::collections::HashMap;
use starlane_resources::message::{Message, ResourcePortMessage};
use wasm_membrane_guest::membrane::{membrane_consume_string, log};
use mechtron_common::{MechtronCall, MechtronCommand};
use lazy_static::lazy_static;
use wasm_bindgen::prelude::*;

#[macro_use]
extern crate wasm_bindgen;

#[macro_use]
extern crate lazy_static;


lazy_static! {
    pub static ref MECHTRONS : RwLock<HashMap<String,Arc<dyn Mechtron>>> = RwLock::new(HashMap::new());
}

extern "C"
{
    //pub fn membrane_host_panic(buffer: i32);
}

pub fn mechtron_register( mechtron: Arc<dyn Mechtron> ) {
    let mut lock = MECHTRONS.write().unwrap();
    lock.insert( mechtron.name(), mechtron );
}

pub fn mechtron_get(name: String) -> Arc<dyn Mechtron> {
    let lock = MECHTRONS.read().unwrap();
    lock.get(&name).cloned().expect(format!("failed to get mechtron named: {}",name).as_str() )
}

#[wasm_bindgen]
pub fn mechtron_call( call: i32 ) {
    let call = match membrane_consume_string(call) {
        Ok(json) => {
            let call: MechtronCall = match serde_json::from_str(json.as_str()) {
                Ok(call) => call,
                Err(error) => {
                    log(format!("mechtron call serialization error: {}",error.to_string()).as_str());
                    return;
                }
            };

            let mechtron: Arc<dyn Mechtron> = {
                let read = MECHTRONS.read().unwrap();
                read.get(&call.mechtron).cloned().expect(format!("expected mechtron: {}",call.mechtron).as_str() )
            };

            match call.command {
                MechtronCommand::Message(message) => {
                    let delivery = Delivery{
                        message
                    };

                    log("delivered message to mechtron within Wasm");
                    mechtron.message(delivery);

                }
            }

        }
        Err(_) => {}
    };
}

pub struct Delivery {
    pub message: Message<ResourcePortMessage>
}

impl Delivery {
}

pub trait Mechtron: Sync+Send+'static {
    fn name(&self) -> String;

    fn message( &self, delivery: Delivery ) {

    }
}