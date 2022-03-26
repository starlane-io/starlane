use wasm_membrane_guest::membrane::log;
use mechtron::{Mechtron, mechtron_register, MechtronCtx, MechtronFactory};
use mechtron::error::Error;
use mesh_portal::version::latest::entity::request::Action;
use mesh_portal::version::latest::entity::response::ResponseCore;
use mesh_portal::version::latest::http::HttpRequest;
use mesh_portal::version::latest::resource::ResourceStub;


//! mechtron_init() is called after the Wasm file has been compiled by Starlane.
//! It's most important job is to register any mechtron factories.
#[no_mangle]
pub extern "C" fn mechtron_init()
{
    //! Here we register the MyAppFactory for creating 'my-app' Mechtrons
    mechtron_register(Box::new(MyAppFactory::new()));
}

//! Factory implementation for MyApp Mechtron
pub struct MyAppFactory { }

impl MyAppFactory {
    pub fn new() -> Self {
        Self{}
    }
}

impl MechtronFactory for MyAppFactory {
    //! Here we returning the very important Mechtron name which will must be referenced in the App config
    fn mechtron_name(&self) -> String {
        "my-app".to_string()
    }

    fn create(&self, stub: ResourceStub) -> Result<Box<dyn Mechtron>, Error> {
        Ok(Box::new(MyApp::new()))
    }
}


//! MyApp mechtron is an implementation of the Mechtron trait.
//! It handles requests and produces Responses
pub struct MyApp {}

impl MyApp {
    pub fn new()->Self{
        Self{}
    }
}

impl Mechtron for MyApp  {

    //! Method for handling Http requests (other types of requests will result in an error response.
    //! Here we are just responding 'Hello World!' to any request that comes in
    fn handle_http_request(&self, ctx: &dyn MechtronCtx, request: HttpRequest ) -> Result<ResponseCore,Error> {
        Ok(request.ok("Hello World!"))
    }

}

