#[macro_use]
extern crate wasm_bindgen;

use wasm_membrane_guest::membrane::log;
use mechtron::{Mechtron, mechtron_register};
use std::sync::Arc;
use starlane_resources::data::BinSrc;
use starlane_resources::http::{HttpRequest, HttpResponse, Headers};
use std::convert::TryInto;
use starlane_resources::message::{ResourcePortReply, ResourcePortMessage, Message, MessageReply};
use std::collections::HashMap;


#[no_mangle]
pub extern "C" fn mechtron_init()
{
    log("********F Hello World! From: Wasm!*************** ");
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
        todo!()
    }

    fn message(&self, message: Message<HttpRequest>) -> Option<HttpResponse> {
        todo!()
    }
}


/*
fn http_request(&self, message: Message<HttpRequest>) -> Option<HttpResponse> {
    log("http_request called on appy ");

//        log(format!("request path: {}",message.payload.path).as_str() );

    let response = HttpResponse{
        status: 200,
        headers: Headers::new(),
        body: Option::Some(BinSrc::Memory(Arc::new("Hello from a Mechtron!".to_string().into_bytes())))
    };

    Option::Some(response)

}

 */
