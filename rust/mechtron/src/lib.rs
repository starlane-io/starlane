use std::sync::{RwLock, Arc};
use std::collections::HashMap;
use starlane_resources::message::{Message, ResourcePortMessage};
lazy_static! {
    pub static ref MECHTRONS : RwLock<HashMap<String,Arc<dyn Mechtron>>> = RwLock::new(HashMap::new());
}


pub fn mechtron_register( mechtron: Arc<dyn Mechtron> ) {
    let mut lock = MECHTRONS.write();
    lock.insert( mechtron.name(), mechtron );
}

pub fn mechtron_get(name: String) -> Arc<dyn Mechtron> {
    let mut lock = MECHTRONS.read();
    lock.get(&name).cloned().expect(format!("failed to get mechtron named: {}",name))
}

pub struct Delivery {
    pub message: Message<ResourcePortMessage>
}

impl Delivery {
    pub fn reply(reply: Reply) {
       // let proto = ProtoMessage::
    }
}

pub trait Mechtron {
    fn name(&self) -> String;

    fn message( &self, delivery: Delivery ) {

    }
}