use serde::{Serialize,Deserialize};
use starlane_resources::message::{ResourcePortMessage, Message};

#[derive(Clone,Serialize,Deserialize)]
pub struct MechtronCall {
    pub mechtron: String,
    pub command: MechtronCommand
}


#[derive(Clone,Serialize,Deserialize)]
pub enum MechtronCommand {
    Message(Message<ResourcePortMessage>)
}