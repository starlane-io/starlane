//#[macro_use]
//extern crate wasm_bindgen;

use wasm_membrane_guest::membrane::log;
use mechtron::{Mechtron, mechtron_register, MechtronCtx, MechtronFactory};
use std::sync::Arc;
use mechtron::error::Error;
use mesh_portal_serde::version::latest::entity::request::Action;
use mesh_portal_serde::version::latest::entity::response::ResponseCore;
use mesh_portal_serde::version::latest::http::HttpRequest;
use mesh_portal_serde::version::latest::messaging::Request;
use mesh_portal_serde::version::latest::payload::{Payload, Primitive};
use mesh_portal_serde::version::latest::resource::ResourceStub;


#[no_mangle]
pub extern "C" fn mechtron_init()
{
    log("********F Hello World! From: Wasm!*************** ");
    mechtron_register(Arc::new(MyAppFactory::new()));
}

pub struct MyAppFactory { }

impl MyAppFactory {
    pub fn new() -> Self {
        Self{}
    }
}

impl MechtronFactory for MyAppFactory {
    fn mechtron_name(&self) -> String {
        "my-app".to_string()
    }

    fn create(&self, stub: ResourceStub) -> Result<Box<dyn Mechtron>, Error> {
        Ok(Box::new(MyApp::new()))
    }
}


pub struct MyApp {}

impl MyApp {
    pub fn new()->Self{
        Self{}
    }
}

impl Mechtron for MyApp  {

    fn handle_http_request(&self, ctx: &dyn MechtronCtx, request: HttpRequest ) -> Result<ResponseCore,Error> {
        Ok(request.ok("Hello World!"))
    }

}

