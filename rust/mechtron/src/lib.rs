use std::collections::HashMap;
use std::convert::TryInto;
use std::sync::{Arc, RwLock};
use std::sync::atomic::{AtomicUsize, Ordering};

use bincode::ErrorKind;
use lazy_static::lazy_static;
use mesh_portal_api::message::Message;
use mesh_portal_serde::version::latest::command::CommandEvent;
use mesh_portal_serde::version::latest::config::Info;
use mesh_portal_serde::version::latest::frame::{CloseReason, PrimitiveFrame};
use mesh_portal_serde::version::latest::id::Address;
use mesh_portal_serde::version::latest::id::Identifier;
use mesh_portal_serde::version::latest::messaging::{Exchange, ExchangeType};
use mesh_portal_serde::version::latest::payload::{Payload, PayloadDelivery, PayloadPattern, PayloadRef};
use mesh_portal_serde::version::latest::portal::inlet;
use mesh_portal_serde::version::latest::portal::outlet;
use mesh_portal_serde::version::latest::portal::outlet::Frame;
use wasm_bindgen::prelude::*;

use error::Error;
use mechtron_common::version::latest::core;
use mechtron_common::version::latest::shell;
use wasm_membrane_guest::membrane::{log, membrane_consume_buffer, membrane_consume_string, membrane_guest_alloc_buffer, membrane_write_buffer, membrane_write_str};
use mesh_portal_serde::version::latest::resource::Status;
use mesh_portal_serde::version::latest::fail::Fail;
use mechtron_common::version::v0_0_1::shell::generic::Response;

mod error;

#[macro_use]
extern crate wasm_bindgen;

#[macro_use]
extern crate lazy_static;

lazy_static! {
    pub static ref FACTORIES: RwLock<HashMap<Address,Arc<dyn MechtronFactory >>> = RwLock::new(HashMap::new());
    pub static ref MECHTRONS: RwLock<HashMap<Address,Arc<MechtronWrapper>>> = RwLock::new(HashMap::new());
    pub static ref MECHTRON_KEY_TO_ADDRESS: RwLock<HashMap<String,Address>> = RwLock::new(HashMap::new());
    pub static ref EXCHANGE_INDEX : AtomicUsize = AtomicUsize::new(0);
}

extern "C"
{
    pub fn mechtron_init();
    pub fn mechtron_host_frame(frame_buffer_id: i32) -> i32;
}


pub fn mechtron_request_notify(request: shell::Request ) -> Result<Option<core::Response>,Error>{
    let request = shell::Request {
        to: request.to,
        from: request.from,
        entity: request.entity,
        exchange: ExchangeType::Notification
    };
    send_request(request)
}

pub fn mechtron_request_exchange(request: shell::Request ) -> Result<core::Response,Error> {
    let request = shell::Request {
        to: request.to,
        from: request.from,
        entity: request.entity,
        exchange: ExchangeType::RequestResponse
    };
    let response = send_request(request);
    let response = match response {
        Ok(response) => {
            let result:Result<core::Response,Error> = response.ok_or("expected response from an exchange message request".into() );
            let response = result?;
            Ok(response)
        }
        Err(err) => {Err(err)}
    };
    response
}

fn send_request(request: shell::Request ) -> Result<Option<core::Response>,Error> {
    let frame = shell::Frame::Request(request);
    let buffer = bincode::serialize(&frame )?;
    let buffer = membrane_write_buffer(buffer );
    let response = unsafe {
        mechtron_host_frame(buffer)
    };
    if response == 0 {
        return Ok(Option::None);
    } else if response < 0 {
        return Err( "an error prevented message response".into() );
    }
    let response = membrane_consume_buffer(response )?;
    let response: core::Response = bincode::deserialize(response.as_slice() )?;
    Ok(Option::Some(response))
}

pub fn mechtron_register(factory: Arc<dyn MechtronFactory> ) {
log(format!("REGISTERED MECHTRON FACTORY: {}", factory.config_address().to_string()).as_str());
    let mut lock = FACTORIES.write().unwrap();
    lock.insert(factory.config_address(), factory);
}

pub fn mechtron_get(address: Address) -> Arc<MechtronWrapper> {
    let lock = MECHTRONS.read().unwrap();
    lock.get(&address).cloned().expect(format!("failed to get mechtron with address: {}",address.to_string()).as_str() )
}

#[wasm_bindgen]
pub fn mechtron_guest_frame(frame_buffer_id: i32 ) -> i32 {
    log("received mechtron call");
    fn mechtron_guest_frame_inner(frame_buffer_id: i32) -> Result<i32, Error> {
        let call = membrane_consume_buffer(frame_buffer_id)?;
        let frame: core::Frame = bincode::deserialize(call.as_slice())?;
        match frame {
            core::Frame::Version(_) => {
                unsafe {
                    mechtron_init();
                }
                Ok(0)
            }
            core::Frame::Create(info) => {
                let factory: Arc<dyn MechtronFactory> = {
                    let read = FACTORIES.read()?;
                    read.get(info.archetype.config_src.as_ref().expect("mechtrons must have a config")).cloned().expect(format!("expected mechtron: ..." ).as_str())
                };

                let mechtron = factory.create(info.clone())?;
                let mechtron = MechtronWrapper::new(info, mechtron);
                {
                    let mut write = MECHTRON_KEY_TO_ADDRESS.write()?;
                    write.insert(mechtron.info.key.clone(), mechtron.info.address.clone() );
                }
                {
                    let mut write = MECHTRONS.write()?;
                    write.insert(mechtron.info.address.clone(), Arc::new(mechtron));
                }


                Ok(0)
            }
            core::Frame::Assign(info) => {
                let factory: Arc<dyn MechtronFactory> = {
                    let read = FACTORIES.read()?;
                    read.get(info.archetype.config_src.as_ref().expect("mechtrons must have a config")).cloned().expect(format!("expected mechtron: ..." ).as_str())
                };

                let mechtron = factory.assign(info.clone())?;
                let mechtron = MechtronWrapper::new(info, mechtron);
                {
                    let mut write = MECHTRON_KEY_TO_ADDRESS.write()?;
                    write.insert(mechtron.info.key.clone(), mechtron.info.address.clone() );
                }
                {
                    let mut write = MECHTRONS.write()?;
                    write.insert(mechtron.info.address.clone(), Arc::new(mechtron));
                }

                Ok(0)
            }
            core::Frame::Destroy(address) => {
                let mechtron = {
                    let mut write = MECHTRONS.write()?;
                    write.remove(&address )
                };
                if let Some(mechtron) = mechtron {
                    let mut write = MECHTRON_KEY_TO_ADDRESS.write()?;
                    write.remove(&mechtron.info.key );
                }
                Ok(0)
            }
            core::Frame::Request(request) => {

                let address = match &request.to {
                    Identifier::Key(key) => {
                        let mut read = MECHTRON_KEY_TO_ADDRESS.read()?;
                        let result: Result<&Address,Error> = read.get(key ).ok_or(format!("could not find key {}",key).into());
                        let address: Address = result?.clone();
                        address
                    }
                    Identifier::Address(address) => {
                        address.clone()
                    }
                };

                let mechtron = {
                    let read = MECHTRONS.read()?;
                    read.get(&address ).cloned()
                };

                match mechtron {
                    None => {
                        log(format!("mechtron not found: '{}'",request.to.to_string()).as_str());
                        return Ok(-1);
                    }
                    Some(mechtron) => {
                        match mechtron.handle(request) {
                            Ok(response) => {
                               match response {
                                   None => {
                                       Ok(0)
                                   }
                                   Some(response) => {
                                       let frame = shell::Frame::Respond(response);
                                       let buffer = bincode::serialize(&frame )?;
                                       let buffer = membrane_write_buffer(buffer );
                                       let response = unsafe {
                                           mechtron_host_frame(buffer)
                                       };
                                       Ok(response)
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
        }
    }

    match mechtron_guest_frame_inner(frame_buffer_id) {
        Ok(response_buffer_id) => {response_buffer_id}
        Err(error) => {
            log( error.to_string().as_str() );
            -1
        }
    }

}



pub trait MechtronFactory: Sync+Send+'static {
    fn config_address(&self) -> Address;

    fn create(&self, info: Info) -> Result<Box<dyn Mechtron>,Error>;
    fn assign(&self, info: Info) -> Result<Box<dyn Mechtron>,Error>;
}




pub struct MechtronWrapper {
    pub info: Info,
    pub mechtron: Box<dyn Mechtron>,
}

impl MechtronWrapper {

    pub fn new( info: Info, mechtron: Box<dyn Mechtron> ) -> Self {
        Self {
            info,
            mechtron
        }
    }


    pub fn handle(&self, request: core::Request ) -> Result<Option<shell::Response>,Fail> {
        self.mechtron.handle(self, request )
    }

    pub fn close( &self, close: CloseReason ) {
        self.mechtron.close(close)
    }
}

impl MechtronCtx for MechtronWrapper {
    fn info(&self) -> &Info {
        &self.info
    }
}


pub trait MechtronCtx  {
    fn info(&self) -> &Info;

    fn notify( &self, request: shell::Request)  {
        mechtron_request_notify(request).unwrap_or_default();
    }

    fn exchange(&self, request: shell::Request) -> Result<core::Response,Error>  {
        mechtron_request_exchange(request)
    }
}


pub trait Mechtron: Sync+Send+'static {

    fn handle(&self, ctx: &dyn MechtronCtx, request: core::Request ) -> Result<Option<shell::Response>,Fail> {
        Ok( Option::Some(request.ok(Payload::Empty)) )
    }

    fn close( &self, close: CloseReason ) {

    }
}


pub struct Panic {
  pub fail: Fail,
  pub message: String
}

#[cfg(test)]
pub mod test {
    #[test]
    pub fn test () {

    }
}
