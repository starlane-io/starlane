use std::sync::{RwLock, Arc};
use std::collections::HashMap;
use wasm_membrane_guest::membrane::{membrane_consume_string, log, membrane_guest_alloc_buffer, membrane_write_str, membrane_write_buffer, membrane_consume_buffer};
use mechtron_common::{MechtronGuestCall, HostToGuestFrame, GuestToHostFrame};
use lazy_static::lazy_static;
use wasm_bindgen::prelude::*;
use mesh_portal_api::message::Message;
use mesh_portal_serde::version::latest::portal::inlet;
use mesh_portal_serde::version::latest::portal::outlet;
use mesh_portal_serde::version::v0_0_1::generic::payload::{PayloadDelivery, PayloadRef, PayloadPattern, Payload};
use mesh_portal_serde::version::v0_0_1::generic::portal::outlet::Frame;
use mesh_portal_serde::version::v0_0_1::generic::id::Identifier;
use mesh_portal_serde::version::v0_0_1::id::Address;
use mesh_portal_serde::version::latest::config::Info;
use mesh_portal_serde::version::latest::command::CommandEvent;
use mesh_portal_serde::version::latest::frame::{CloseReason, PrimitiveFrame};
use std::convert::TryInto;
use std::sync::atomic::{AtomicUsize, Ordering};
use mesh_portal_serde::version::latest::messaging::Exchange;

#[macro_use]
extern crate wasm_bindgen;

#[macro_use]
extern crate lazy_static;




lazy_static! {
    pub static ref MECHTRONS : RwLock<HashMap<String,Arc<dyn Mechtron>>> = RwLock::new(HashMap::new());
    pub static ref EXCHANGE_INDEX : AtomicUsize = AtomicUsize::new();
}


extern "C"
{
    pub fn mechtron_respond(call_id: i32, respond: i32);
    pub fn mechtron_host_call(call_id: i32) -> i32;
}
pub fn mechtron_notify( request: inlet::Request ) -> Result<Option<outlet::Response>,Error>{
    let request = inlet::exchange::Request {
        id: request.id,
        to: request.to,
        entity: request.entity,
        exchange: Exchange::Notification
    };
    send_request(request)
}

pub fn mechtron_request( request: inlet::Request ) -> Result<Option<outlet::Response>,Error> {
    let request = inlet::exchange::Request {
        id: request.id,
        to: request.to,
        entity: request.entity,
        exchange: Exchange::RequestResponse(EXCHANGE_INDEX.fetch_add(1, Ordering::Relaxed).to_string())
    };
    send_request(request)
}

fn send_request( request: inlet::exchange::Request ) -> Result<Option<outlet::Response>,Error> {
    let frame = inlet::Frame::Request( request );
    let call = GuestToHostFrame::MeshPortalFrame(frame);
    let buffer = bincode::serialize(&call)?;
    let buffer = membrane_write_buffer(buffer );
    let response = unsafe {
        mechtron_host_call(buffer)
    };
    if response == 0 {
        return Ok(Option::None);
    }
    let response = membrane_consume_buffer(response)?;
    let response = bincode::deserialize(response.as_slice() )?;
    Ok(Option::Some(response))
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
        Ok(call) => {
log("String consumed");
            let call: MechtronGuestCall = match serde_json::from_str(call.as_str()) {
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

            match call.frame {
                HostToGuestFrame::MeshPortalFrame(frame) => {

                    match frame {
                        Frame::Init(info) => {
                            mechtron.init(info);
                        }
                        Frame::CommandEvent(e) => {
                            mechtron.command_event(e);
                        }
                        Frame::Request(request) => {
                            log("delivered message to mechtron within Wasm");
                            let response = mechtron.request(request);
                            log("delivery complete");

                            match response {
                                None => {
                                    0
                                }
                                Some(reply) => {
                                    let response = bincode::serialize(&response).expect("expected resource port reply to be able to serialize into a bincode");
                                    let response = membrane_write_str(response.as_str() );
                                    log("WASM message reply COMPLETE...");
                                    response
                                }
                            }
                        }
                        Frame::Response(response) => {
                            // we should actually never get a response frame
                            // instead wait for response from host
                            // this frame is useful in other portal implementations
                        }
                        Frame::Close(close) => {
                            mechtron.close(close);
                        }
                    }
                    0
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

    fn init(&self, info: Info) {

    }

    fn command_event(&self, event: CommandEvent  ) {

    }

    fn request(&self, request: outlet::exchange::Request ) -> Option<inlet::Response> {
        Option::None
    }

    fn close( &self, close: CloseReason ) {

    }
}


pub struct Error {
    pub message: String
}

impl From<mesh_portal_serde::version::latest::error::Error> for Error {
    fn from(error: mesh_portal_serde::version::latest::error::Error) -> Self {
        Self {
            message: error.to_string()
        }
    }
}

impl From<wasm_membrane_guest::error::Error> for Error {
    fn from(error: wasm_membrane_guest::error::Error) -> Self {
        Self {
            message: error.to_string()
        }
    }
}



#[cfg(test)]
pub mod test {
    #[test]
    pub fn test () {

    }
}