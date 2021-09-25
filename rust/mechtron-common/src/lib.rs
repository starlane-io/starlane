use serde::{Serialize,Deserialize};
use starlane_resources::message::{ResourcePortMessage, Message, ResourcePortReply};
use starlane_resources::http::{HttpRequest, HttpResponse};

#[derive(Clone,Serialize,Deserialize)]
pub struct MechtronCall {
    pub mechtron: String,
    pub command: MechtronCommand
}


#[derive(Clone,Serialize,Deserialize)]
pub enum MechtronCommand {
    Message(Message<ResourcePortMessage>),
    HttpRequest(Message<HttpRequest>)
}


#[derive(Clone,Serialize,Deserialize)]
pub enum MechtronResponse{
    PortReply(ResourcePortReply),
    HttpResponse(HttpResponse)
}