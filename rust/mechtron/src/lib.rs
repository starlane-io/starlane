pub mod error;

#[macro_use]
extern crate wasm_bindgen;

#[macro_use]
extern crate lazy_static;

use crate::error::Error;
use mechtron_common::outlet;
use mechtron_common::outlet::Frame;
use mesh_portal_serde::version::latest::config::ResourceConfigBody;
use mesh_portal_serde::version::latest::entity::request::Action;
use mesh_portal_serde::version::latest::entity::response::ResponseCore;
use mesh_portal_serde::version::latest::frame::CloseReason;
use mesh_portal_serde::version::latest::http::HttpRequest;
use mesh_portal_serde::version::latest::id::Address;
use mesh_portal_serde::version::latest::messaging::{ProtoRequest, Request, Response};
use mesh_portal_serde::version::latest::msg::MsgRequest;
use mesh_portal_serde::version::latest::payload::{Errors, Payload, Primitive};
use mesh_portal_serde::version::latest::resource::ResourceStub;
use mesh_portal_serde::version::latest::util::unique_id;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use std::sync::RwLock;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_membrane_guest::membrane::{
    log, membrane_consume_buffer, membrane_read_buffer, membrane_write_buffer,
};

lazy_static! {
    pub static ref FACTORIES: RwLock<HashMap<String, Arc<dyn MechtronFactory>>> =
        RwLock::new(HashMap::new());
    pub static ref MECHTRONS: RwLock<HashMap<Address, Arc<MechtronWrapper>>> =
        RwLock::new(HashMap::new());
    pub static ref EXCHANGE_INDEX: AtomicUsize = AtomicUsize::new(0);
}

extern "C" {
    pub fn mechtron_init();
    pub fn mechtron_inlet_frame(frame: i32);
    pub fn mechtron_inlet_request(request: i32) -> i32;
}

#[wasm_bindgen]
pub fn mechtron_outlet_request(request: i32) -> i32 {
    let request = match membrane_read_buffer(request) {
        Ok(request) => {
            match bincode::deserialize(request.as_slice()) {
                Ok(request) => {
                    // this is done for the sake of bindcode which requires an explicit cast
                    let request: Request = request;
                    let mechtron = {
                        let read = MECHTRONS.read().expect("expect read access to Mechrons");
                        read.get(&request.to).cloned()
                    };
                    match mechtron {
                        None => {
                            let message =
                                format!("mechtron not found: '{}'", request.to.to_string());
                            log(message.as_str());
                            return mechtron_write_response(request.fail(message.as_str()));
                        }
                        Some(mechtron) => {
                            let response = mechtron.handle(request);
                            return mechtron_write_response(response);
                        }
                    }
                }
                Err(err) => {
                    log(err.to_string().as_str());
                    return -1;
                }
            }
        }
        Err(err) => {
            log(err.to_string().as_str());
            return -1;
        }
    };
}
#[wasm_bindgen]
pub fn mechtron_outlet_frame(frame_buffer_id: i32) {
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
                match assign.config.body {
                    ResourceConfigBody::Control => {
                        log("mechtron framework cannot create a Control")
                    }
                    ResourceConfigBody::Named(mechtron_name) => {
                        let factory: Arc<dyn MechtronFactory> = {
                            let factories = FACTORIES.read()?;
                            factories.get(&mechtron_name).ok_or(format!(""))?.clone()
                        };

                        let mechtron = factory.create(assign.stub.clone())?;
                        let mechtron = MechtronWrapper::new(assign.stub.clone(), mechtron);
                        {
                            let mut write = MECHTRONS.write()?;
                            write.insert(assign.stub.address.clone(), Arc::new(mechtron));
                        }
                    }
                }
                Ok(())
            }
            Frame::ArtifactResponse(response) => Ok(()),
        }
    }

    match mechtron_outlet_frame_inner(frame_buffer_id) {
        Ok(_) => {}
        Err(error) => {
            log(error.to_string().as_str());
        }
    }
}

fn mechtron_send_inlet_request(request: Request) -> Response {
    let request_buffer_id = mechtron_write_request(request.clone());
    let response = unsafe { mechtron_inlet_request(request_buffer_id) };
    if response == 0 {
        return request.fail("request returned no response from host");
    } else if response < 0 {
        return request.fail("an error prevented message response");
    }

    let response = match mechtron_read_response(response) {
        Ok(response) => response,
        Err(err) => { request.fail(err.to_string().as_str()) }
    };
    response
}

pub fn mechtron_register(factory: Arc<dyn MechtronFactory>) {
    log(format!(
        "REGISTERED MECHTRON FACTORY: '{}'",
        factory.mechtron_name()
    )
    .as_str());
    let mut lock = FACTORIES.write().unwrap();
    lock.insert(factory.mechtron_name(), factory);
}

fn mechtron_get(address: Address) -> Result<Arc<MechtronWrapper>, Error> {
    let lock = MECHTRONS.read().unwrap();
    Ok(lock.get(&address).cloned().ok_or(format!(
        "failed to get mechtron with address: {}",
        address.to_string()
    ))?)
}

fn mechtron_write_response(response: Response) -> i32 {
    let buffer = bincode::serialize(&response).expect("expected to be able to serialize response");
    membrane_write_buffer(buffer)
}

fn mechtron_write_request(request: Request) -> i32 {
    let buffer = bincode::serialize(&request).expect("expected to be able to serialize request");
    membrane_write_buffer(buffer)
}

fn mechtron_read_response(response: i32) -> Result<Response,Error> {
    let response = membrane_consume_buffer(response)?;
    let response: Response = bincode::deserialize(response.as_slice())?;
    Ok(response)
}

fn mechtron_read_request(request: i32) -> Result<Request,Error>{
    let request = membrane_consume_buffer(request)?;
    let request: Request = bincode::deserialize(request.as_slice())?;
    Ok(request)
}

pub trait MechtronFactory: Sync + Send + 'static {
    fn mechtron_name(&self) -> String;
    fn create(&self, stub: ResourceStub) -> Result<Box<dyn Mechtron>, Error>;
}

pub struct MechtronWrapper {
    pub stub: ResourceStub,
    pub mechtron: Box<dyn Mechtron>,
}

impl MechtronWrapper {
    pub fn new(stub: ResourceStub, mechtron: Box<dyn Mechtron>) -> Self {
        Self { stub, mechtron }
    }

    pub fn handle(&self, request: Request) -> Response {
        let core = self.mechtron.handle(self, request.clone());
        match core {
            Ok(core) => core.into_response(self.stub.address.clone(), request.from, request.id),
            Err(err) => {
                // here we should also set the Status to a Panic state
                request.fail(err.to_string().as_str())
            }
        }
    }

    pub fn close(&self, close: CloseReason) {
        self.mechtron.destroy();
    }
}

impl MechtronCtx for MechtronWrapper {
    fn stub(&self) -> &ResourceStub {
        &self.stub
    }
}

pub trait MechtronCtx {
    fn stub(&self) -> &ResourceStub;

    fn send(&self, request: ProtoRequest) -> Response {
        match request.clone().into_request(self.stub().address.clone()) {
            Ok(request) => mechtron_send_inlet_request(request),
            Err(err) => Response {
                id: unique_id(),
                from: self.stub().address.clone(),
                to: self.stub().address.clone(),
                core: ResponseCore::fail(err.to_string().as_str()),
                response_to: request.id.clone(),
            },
        }
    }
}

pub trait Mechtron: Sync + Send + 'static {
    fn handle(&self, ctx: &dyn MechtronCtx, request: Request) -> Result<ResponseCore, Error> {
        match request.core.action {
            Action::Rc(_) => Ok(request.core.fail(
                format!(
                    "Mechtron {} does not handle Rc actions",
                    ctx.stub().address.to_string()
                )
                .as_str(),
            )),
            Action::Msg(_) => self.handle_msg_request(ctx, MsgRequest::try_from(request.core)?),
            Action::Http(_) => self.handle_http_request(ctx, HttpRequest::try_from(request.core)?),
        }
    }

    fn handle_msg_request(
        &self,
        ctx: &dyn MechtronCtx,
        request: MsgRequest,
    ) -> Result<ResponseCore, Error> {
        Ok(request.fail(format!(
            "Mechtron '{}' does not have a Msg handler implementation",
            ctx.stub().address.to_string()
        ).as_str()))
    }

    fn handle_http_request(
        &self,
        ctx: &dyn MechtronCtx,
        request: HttpRequest,
    ) -> Result<ResponseCore, Error> {
        Ok(request.fail(format!(
            "Mechtron '{}' does not have an Http handler implementation",
            ctx.stub().address.to_string()
        ).as_str()))
    }

    fn destroy(&self) {}
}

pub trait HttpMechtron: Mechtron {}

#[cfg(test)]
pub mod test {
    #[test]
    pub fn test() {}
}
