#[macro_use]
extern crate wasm_bindgen;

use wasm_membrane_guest::membrane::log;
use mechtron::{Mechtron, mechtron_register};
use std::sync::Arc;
use starlane_resources::data::BinSrc;
use starlane_resources::http::{HttpRequest, HttpResponse, Headers};
use std::convert::TryInto;
use starlane_resources::message::{ResourcePortReply, ResourcePortMessage, Message};
use std::collections::HashMap;


#[no_mangle]
pub extern "C" fn mechtron_init()
{
    log("Hello World! From: Wasm!");
    mechtron_register(Arc::new(Appy::new()));
}


pub struct Appy {

}

impl Appy {
    pub fn new()->Self{
        Self{}
    }
}

impl Mechtron for Appy  {
    fn name(&self) -> String {
        "appy".to_string()
    }

    fn deliver( &self, message: Message<ResourcePortMessage>) -> Option<ResourcePortReply>{
        log("Delivery of Message to Appy mechtron");
        let request = message.payload.payload.get("request").cloned().expect("expected request");
        let request : HttpRequest = request.try_into().expect("expect to be able to change to HttpRequest");

        log(format!("request path: {}",request.path).as_str() );

        let response = HttpResponse{
            status: 200,
            headers: Headers::new(),
            body: Option::Some(BinSrc::Memory(Arc::new("Hello from a Mechtron!".to_string().into_bytes())))
        };

        let response :BinSrc =  response.try_into().expect("expect an httpResponse to be able to turn into a BinSrc");
        let mut payload = HashMap::new();
        payload.insert( "response".to_string(), response );

        let reply = ResourcePortReply {
            payload: payload
        };
        Option::Some(reply)
    }

}


