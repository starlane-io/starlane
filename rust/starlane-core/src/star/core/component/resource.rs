

use crate::util::{AsyncProcessor, AsyncRunner};
use crate::frame::StarMessage;
use crate::star::StarSkel;
use tokio::sync::mpsc;
use crate::resource::{ResourceKey, ResourceType, Path};
use crate::data::{BinSrc, DataSet};
use crate::error::Error;
use std::collections::HashMap;
use tokio::sync::oneshot;
use std::sync::Arc;
use std::str::FromStr;


pub enum StateStoreCall {
    Save{key: ResourceKey, state: DataSet<BinSrc>, tx: oneshot::Sender<Result<(),Error>> },
    Get{key: ResourceKey, tx: oneshot::Sender<Result<DataSet<BinSrc>,Error>> }
}

pub struct StateStore {
    skel: StarSkel,
}

impl StateStore {
    pub fn new(skel: StarSkel) -> mpsc::Sender<StateStoreCall> {
        let (tx,rx) = mpsc::channel(1024);

        AsyncRunner::new(Self{
            skel: skel
        },tx.clone(), rx);

        tx
    }

    async fn save( &mut self, key: ResourceKey, state:DataSet<BinSrc> ) -> Result<(),Error>{
        let key = key.to_string();

        let state_path= Path::from_str(format!("/stars/{}/states/{}",self.skel.info.key.to_string(), key).as_str())?;
        let machine_filesystem = self.skel.machine.machine_filesystem();
        let mut data_access = machine_filesystem.data_access();
        data_access.mkdir(&state_path).await?;

        let mut rxs = vec![];
        for (aspect, bin) in state {
            let state_aspect_file = Path::from_str(format!("/stars/{}/states/{}/{}",self.skel.info.key.to_string(),key.to_string(),aspect).as_str())?;
            let (tx,rx) = oneshot::channel();
            let bin_context = self.skel.machine.bin_context();
            bin.mv(bin_context, state_aspect_file, tx ).await;
            rxs.push(rx);
        }

        for rx in rxs {
            rx.await?;
        }

        Ok(())
    }

    async fn get( &self, key: ResourceKey ) -> Result<DataSet<BinSrc>,Error> {
        let key = key.to_string();
        let machine_filesystem = self.skel.machine.machine_filesystem();
        let mut data_access = machine_filesystem.data_access();

        let state_path= Path::from_str(format!("/stars/{}/states/{}",self.skel.info.key.to_string(), key).as_str())?;
        let mut dataset = DataSet::new();
        for aspect in data_access.list(&state_path).await? {
            let state_aspect_file = Path::from_str(format!("/stars/{}/states/{}/{}",self.skel.info.key.to_string(),key.to_string(),aspect.last_segment().ok_or("expected final segment from list")?).as_str())?;
            let bin= data_access.read(&state_aspect_file).await?;
            let bin_src = BinSrc::Memory(bin);
            dataset.insert( aspect.last_segment().ok_or("expected final segment from list")?, bin_src );
        }
        Ok(dataset)
    }

}

impl AsyncProcessor<StateStoreCall> for StateStore {
    async fn process(&mut self, call: StateStoreCall) {
        match call {
            StateStoreCall::Save { key, state, tx } => {
                tx.send( self.save( key, state ).unwrap_or_default());
            }
            StateStoreCall::Get { key, tx } => {
                tx.send( self.get( key).unwrap_or_default() );
            }
        }
    }
}


