use std::collections::HashMap;
use std::convert::TryInto;
use std::sync::{Arc, RwLock};
use std::sync::atomic::{AtomicUsize, Ordering};

use bincode::ErrorKind;
use lazy_static::lazy_static;
use mesh_portal_api::message::Message;
use mesh_portal_serde::version::latest::command::CommandEvent;
use mesh_portal_serde::version::latest::config::{Info, ResourceConfigBody};
use mesh_portal_serde::version::latest::entity::response::ResponseCore;
use mesh_portal_serde::version::latest::frame::{CloseReason, PrimitiveFrame};
use mesh_portal_serde::version::latest::id::Address;
use mesh_portal_serde::version::latest::id::Identifier;
use mesh_portal_serde::version::latest::messaging::{Exchange, ExchangeType, ProtoRequest, Request, Response};
use mesh_portal_serde::version::latest::payload::{Errors, Payload, PayloadDelivery, PayloadPattern, PayloadRef, Primitive};
use wasm_bindgen::prelude::*;

use error::Error;
use mechtron_common::version::latest::core;
use mechtron_common::version::latest::shell;
use mechtron_common::outlet;
use mechtron_common::inlet;
use wasm_membrane_guest::membrane::{log, membrane_consume_buffer, membrane_consume_string, membrane_guest_alloc_buffer, membrane_read_buffer, membrane_write_buffer, membrane_write_str};
use mesh_portal_serde::version::latest::resource::{ResourceStub, Status};
use mesh_portal_serde::version::latest::fail::Fail;
use mesh_portal_serde::version::latest::util::unique_id;
use mechtron_common::outlet::Frame;
use mechtron_common::version::v0_0_1::shell::generic::Response;

mod error;

#[macro_use]
extern crate wasm_bindgen;

#[macro_use]
extern crate lazy_static;

lazy_static! {
    pub static ref FACTORIES: RwLock<HashMap<String,Arc<dyn MechtronFactory >>> = RwLock::new(HashMap::new());
    pub static ref MECHTRONS: RwLock<HashMap<Address,Arc<MechtronWrapper>>> = RwLock::new(HashMap::new());
    pub static ref EXCHANGE_INDEX : AtomicUsize = AtomicUsize::new(0);
}

extern "C"
{
    pub fn mechtron_init();
    pub fn mechtron_inlet_frame(frame: i32);
    pub fn mechtron_inlet_request(request: i32) -> i32;
}


#[wasm_bindgen]
pub fn mechtron_outlet_request(request: i32 ) -> i32{

    let request = match membrane_read_buffer(request) {
        Ok(request) => {
            match bincode::deserialize(request.as_slice()) {
                Ok(request) => {
                    // this is done for the sake of bindcode which requires an explicit cast
                    let request : Request = request;
                    let mechtron = {
                        let read = MECHTRONS.read()?;
                        read.get(&request.to).cloned()
                    };
                    match mechtron {
                        None => {
                            let message = format!("mechtron not found: '{}'",request.to.to_string());
                            log(message.as_str());
                            return mechtron_write_response(request.core.fail(message) );
                        }
                        Some(mechtron) => {
                            match mechtron.handle(request) {
                                Ok(response) => {
                                    match response {
                                        None => {
                                            write_r
                                        }
                                        Some(response) => {
                                            let frame = shell::Frame::Respond(response);
                                            let buffer = bincode::serialize(&frame )?;
                                            let buffer = membrane_write_buffer(buffer );
                                            unsafe {
                                                mechtron_inlet_frame(buffer)
                                            };
                                        }
                                    }
                                }
                                Err(panic) => {
                                    // not sure how to handle this yet
                                    Err("mechtron panic".into())
                                }
                            }
                        }
                    }
                }
                Err(err) => {
                    log( err.to_string().as_str() );
                    return 0;
                }
            }
        }
        Err(err) => {
            log( err.to_string().as_str() );
            return 0;
        }
    };

    let mechtron = {
        let read = MECHTRONS.read()?;
        read.get(&request.to).cloned()
    };


}
#[wasm_bindgen]
pub fn mechtron_outlet_frame(frame_buffer_id: i32 ) {
    log("received mechtron outlet frame");

    fn mechtron_outlet_frame_inner(frame_buffer_id: i32) -> Result<(), Error> {
        let call = membrane_consume_buffer(frame_buffer_id)?;
        let frame: outlet::Frame = bincode::deserialize(call.as_slice())?;
        match frame {
            Frame::Init => {
                unsafe {
                    mechtron_init();
                }
                Ok(())
            }
            outlet::Frame::Assign(assign) => {
                match assign.config.body{
                    ResourceConfigBody::Control => {
                        log("mechtron framework cannot create a Control")
                    }
                    ResourceConfigBody::Named(mechtron_name) => {
                        let factory: Arc<dyn MechtronFactory> = {
                            let factories = FACTORIES.read()?;
                            factories.get(&mechtron_name).ok_or(format!(""))?.clone()
                        };

                        let mechtron = factory.create(assign.stub.clone() )?;
                        let mechtron = MechtronWrapper::new(info, mechtron);
                        {
                            let mut write = MECHTRONS.write()?;
                            write.insert(assign.stub.address.clone(), Arc::new(mechtron));
                        }
                    }
                }
                Ok(())
            }
            Frame::ArtifactResponse(response) => {

                Ok(())
            }
        }
    }

    match mechtron_outlet_frame_inner(frame_buffer_id) {
        Ok(response_buffer_id) => {response_buffer_id}
        Err(error) => {
            log( error.to_string().as_str() );
            -1
        }
    }

}

fn mechtron_request(request: Request ) -> Response {
    let request_buffer= bincode::serialize(&request )?;
    let request_buffer = membrane_write_buffer(request_buffer );
    let response = unsafe {
        mechtron_inlet_request(request_buffer)
    };
    if response == 0 {
        return request.fail("request returned no response from host".to_string() );
    } else if response < 0 {
        return request.fail( "an error prevented message response".to_string()() );
    }

    let response = membrane_consume_buffer(response )?;
    let response: Response = bincode::deserialize(response.as_slice() )?;
    response
}

pub fn mechtron_register(factory: Arc<dyn MechtronFactory> ) {
log(format!("REGISTERED MECHTRON FACTORY: {}", factory.config_address().to_string()).as_str());
    let mut lock = FACTORIES.write().unwrap();
    lock.insert(factory.mechtron_name(), factory);
}

fn mechtron_get(address: Address) -> Result<Arc<MechtronWrapper>,Error> {
    let lock = MECHTRONS.read().unwrap();
    Ok(lock.get(&address).cloned().ok_or(format!("failed to get mechtron with address: {}",address.to_string()) )?)
}

fn mechtron_write_response(response: ResponseCore ) -> i32 {
    let buffer = bincode::serialize(&response).expect("expected to be able to serialize response");
    membrane_write_buffer(buffer)
}




pub trait MechtronFactory: Sync+Send+'static {
    fn mechtron_name(&self) -> String;
    fn create(&self, stub: ResourceStub) -> Result<Box<dyn Mechtron>,Error>;
}




pub struct MechtronWrapper {
    pub stub: ResourceStub,
    pub mechtron: Box<dyn Mechtron>,
}

impl MechtronWrapper {

    pub fn new( stub: ResourceStub, mechtron: Box<dyn Mechtron> ) -> Self {
        Self {
            stub,
            mechtron
        }
    }

    pub fn handle(&self, request: Request ) -> Response {
        let core = self.mechtron.handle(self, request.clone() );
        match core {
            Ok(core) => core.into_response(self.stub.address.clone(), request.from, request.id ),
            Err(err) => {
                // here we should also set the Status to a Panic state
                request.fail(err.to_string() )
            }
        }
    }

    pub fn close( &self, close: CloseReason ) {
        self.mechtron.destroy(close)
    }
}

impl MechtronCtx for MechtronWrapper {
    fn stub(&self) -> &Info {
        &self.info
    }
}


pub trait MechtronCtx  {
    fn stub(&self) -> &ResourceStub;

    fn send_request(&self, request: ProtoRequest) -> Response  {
        match request.into_request(self.stub().address.clone() ) {
            Ok(request) => {
                mechtron_request(request)
            }
            Err(err) => {
                Response {
                    id: unique_id(),
                    from: self.stub().address.clone(),
                    to: self.stub().address.clone(),
                    core: ResponseCore {
                        headers: Default::default(),
                        code: 500,
                        body: Payload::Primitive(Primitive::Errors(Errors::default(err.to_string().as_str() )))
                    },
                    response_to: request.id
                }
            }
        }
    }
}


pub trait Mechtron: Sync+Send+'static {

    fn handle(&self, ctx: &dyn MechtronCtx, request: Request ) -> Result<ResponseCore,Error> {
        Ok(request.core.fail(format!("Mechtron {} does not have a message handler implementation",ctx.stub().stub.address.to_string())))
    }

    fn destroy(&self) {

    }
}



#[cfg(test)]
pub mod test {
    #[test]
    pub fn test () {

    }
}
