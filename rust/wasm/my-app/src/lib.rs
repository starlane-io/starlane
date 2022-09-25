#![allow(warnings)]

#[macro_use]
extern crate lazy_static;
//mod html;

use std::sync::Arc;
use cosmic_universe::err::UniErr;
use cosmic_universe::particle::Details;
use mechtron::{DefaultPlatform, Guest, MechtronFactories, MechtronFactory, Platform, guest};
use mechtron::err::{GuestErr, MechErr};
use mechtron::Mechtron;

//use mechtron::error::Error;
/*use mesh_portal::version::latest::entity::request::Action;
use mesh_portal::version::latest::entity::response::ResponseCore;
use mesh_portal::version::latest::http::HttpRequest;
use mesh_portal::version::latest::messaging::Request;
use mesh_portal::version::latest::payload::{Payload, Primitive};
use mesh_portal::version::latest::resource::ResourceStub;

use crate::html::greeting;
 */


#[no_mangle]
pub extern "C" fn mechtron_guest( details: Details ) -> Result<Arc<dyn mechtron::Guest>,GuestErr> {
   Ok(Arc::new(mechtron::guest::Guest::new(details, MyAppPlatform::new() )?))
}

pub struct MyAppPlatform;

impl Platform for MyAppPlatform {
  type Err = GuestErr;
}

impl MyAppPlatform {
  pub fn new() -> Self {
    Self {}
  }
}




/*
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

 */

