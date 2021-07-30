

use crate::util::{AsyncProcessor, AsyncRunner};
use crate::frame::StarMessage;
use crate::star::StarSkel;
use tokio::sync::mpsc;
use crate::resource::{ResourceKey, ResourceType};
use crate::data::{BinSrc, DataSet};
use crate::error::Error;
use std::collections::HashMap;
use tokio::sync::oneshot;
use std::sync::Arc;

pub enum StateStoreCall {
    Save{key: ResourceKey, state: DataSet<BinSrc>, tx: oneshot::Sender<Result<(),Error>> },
    Get{key: ResourceKey, tx: oneshot::Sender<Result<DataSet<BinSrc>,Error>> }
}

pub struct StateStore {
    skel: StarSkel,
    stores: HashMap<ResourceType,Arc<dyn ResourceStateStore>>
}

impl StateStore {
    pub fn new(skel: StarSkel, stores: HashMap<ResouceType,Arc<dyn ResourceStateStore>> ) -> mpsc::Sender<StateStoreCall> {
        let (tx,rx) = mpsc::channel(1024);

        AsyncRunner::new(Self{
            skel: skel
        },tx.clone(), rx);

        tx
    }
}

impl AsyncProcessor<StateStoreCall> for StateStore {
    async fn process(&mut self, call: StateStoreCall) {
        todo!()
    }
}



pub trait ResourceStateStore {
}