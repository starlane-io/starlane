use crate::frame::StarMessage;
use crate::star::StarSkel;
use tokio::sync::mpsc;
use crate::util::{AsyncRunner, AsyncProcessor};

pub mod component;

pub enum CoreCall {
    Message(StarMessage)
}

pub struct Router {
    skel: StarSkel
}

impl Router {
    pub fn new(skel: StarSkel) -> mpsc::Sender<CoreCall> {
        let (tx,rx) = mpsc::channel(1024);

        AsyncRunner::new(Self{
            skel: skel
        },tx.clone(), rx);

        tx
    }
}

impl AsyncProcessor<CoreCall> for Router {
    async fn process(&mut self, call: CoreCall) {
        todo!()
    }
}
