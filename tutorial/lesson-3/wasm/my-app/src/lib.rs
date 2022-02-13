//#[macro_use]
//extern crate wasm_bindgen;

#[macro_use]
extern crate lazy_static;
mod html;

use wasm_membrane_guest::membrane::log;
use mechtron::{Mechtron, mechtron_register, MechtronCtx, MechtronFactory};
use mechtron::error::Error;
use mesh_portal_serde::version::latest::entity::request::Action;
use mesh_portal_serde::version::latest::entity::response::ResponseCore;
use mesh_portal_serde::version::latest::http::HttpRequest;
use mesh_portal_serde::version::latest::messaging::Request;
use mesh_portal_serde::version::latest::payload::{Payload, Primitive};
use mesh_portal_serde::version::latest::resource::ResourceStub;
use crate::html::greeting;


#[no_mangle]
pub extern "C" fn mechtron_init()
{
    mechtron_register(Box::new(MyAppFactory::new()));
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
        let mut name = request.path.clone();
        name.remove(0); // remove leading slash
        match greeting(name.as_str() ) {
            Ok(response) => {
                Ok(response)
            }
            Err(err) => {
                Ok(request.fail("Rendering Error" ))
            }
        }
    }

}

