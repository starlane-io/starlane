use std::sync::{RwLock, Arc};
use std::collections::HashMap;
use starlane_resources::message::{Message, ResourcePortMessage, ResourcePortReply};
use wasm_membrane_guest::membrane::{membrane_consume_string, log, membrane_guest_alloc_buffer, membrane_write_str};
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
    pub fn mechtron_message_reply(call_id: i32, reply: i32);
}

pub fn mechtron_register( mechtron: Arc<dyn Mechtron> ) {
log(format!("REGISTERED MECHTRON: {}", mechtron.name()).as_str());
    let mut lock = MECHTRONS.write().unwrap();
    lock.insert( mechtron.name(), mechtron );
}

pub fn mechtron_get(name: String) -> Arc<dyn Mechtron> {
    let lock = MECHTRONS.read().unwrap();
    lock.get(&name).cloned().expect(format!("failed to get mechtron named: {}",name).as_str() )

}

#[wasm_bindgen]
pub fn mechtron_call(call_id: i32 ) -> i32 {
    log("received mechtron call");
    match membrane_consume_string(call_id) {
        Ok(json) => {
log("String consumed");
            let call: MechtronCall = match serde_json::from_str(json.as_str()) {
                Ok(call) => call,
                Err(error) => {
                    log(format!("mechtron call serialization error: {}",error.to_string()).as_str());
                    return -1;
                }
            };
log(format!("got mechtron call {}", call.mechtron ).as_str());

            let mechtron: Arc<dyn Mechtron> = {
                let read = MECHTRONS.read().unwrap();
                read.get(&call.mechtron).cloned().expect(format!("expected mechtron: {}",call.mechtron).as_str() )
            };
log("GOT MECHTRON ");

            match call.command {
                MechtronCommand::Message(message) => {

                    log("delivered message to mechtron within Wasm");
                    let reply = mechtron.deliver(message);
                    log("delivery complete");

                    match reply {
                        None => {
                            0
                        }
                        Some(reply) => {
                            let reply = serde_json::to_string(&reply).expect("expected resource port reply to be able to serialize into a string");
                            let reply = membrane_write_str(reply.as_str() );
                            log("WASM message reply COMPLETE...");
                            reply
                        }
                    }
                }
            }

        }
        Err(_) => {
            -1
        }
    }
}



pub trait Mechtron: Sync+Send+'static {
    fn name(&self) -> String;

    fn deliver( &self, message: Message<ResourcePortMessage>) -> Option<ResourcePortReply> {
        Option::None
    }
}
