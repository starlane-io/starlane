pub mod html;

#[macro_use]
extern crate wasm_bindgen;

#[macro_use]
extern crate lazy_static;


use wasm_membrane_guest::membrane::log;
use mechtron::{Mechtron, mechtron_register};
use std::sync::Arc;
use starlane_resources::data::BinSrc;
use starlane_resources::http::{HttpRequest, HttpResponse, Headers};
use std::convert::TryInto;
use starlane_resources::message::{ResourcePortReply, ResourcePortMessage, Message};
use std::collections::HashMap;
use starlane_resources::ResourcePath;
use std::str::FromStr;


#[no_mangle]
pub extern "C" fn mechtron_init()
{
    log("Hello World! From: Wasm!");
    mechtron_register(Arc::new(MyMechtron::new()));
}


pub struct MyMechtron {

}

impl MyMechtron {
    pub fn new()->Self{
        Self{}
    }
}

impl Mechtron for MyMechtron {
    fn name(&self) -> String {
        "my-mechtron".to_string()
    }



    fn http_request(&self, message: Message<HttpRequest>) -> Option<HttpResponse> {
        log("http_request called on appy ");

        log(format!("request path: {}",message.payload.path).as_str() );

        let response = html::mechtron_page().unwrap();

        Option::Some(response)

    }

}


