
use std::str::FromStr;
use std::sync::Arc;

use rusqlite::{Connection, params, Row};
use rusqlite::types::ValueRef;
use tokio::sync::{mpsc, oneshot};

use starlane_resources::{ResourceIdentifier, ResourceStatePersistenceManager};

use crate::app::ConfigSrc;
use crate::error::Error;

use crate::message::Fail;
use crate::resource::{LocalStateSetSrc, Resource, ResourceAddress, ResourceArchetype, ResourceAssign, ResourceCreate, ResourceKey, ResourceKind, Specific, Path};
use std::convert::TryInto;
use crate::data::{DataSet, BinSrc};
use crate::file_access::FileAccess;
use crate::starlane::files::MachineFileSystem;
use crate::star::StarSkel;

#[derive(Clone,Debug)]
pub struct StateStore {
    tx: mpsc::Sender<ResourceStoreCommand>,
}

impl StateStore {
    pub async fn new(skel: StarSkel) -> Self {
        StateStore {
            tx: StateStoreFS::new(skel).await,
        }
    }

    pub async fn put(
        &self,
        assign: ResourceAssign<DataSet<BinSrc>>,
    ) -> Result<(), Fail> {
        let (tx, rx) = oneshot::channel();

        self.tx
            .send( ResourceStoreCommand::Save{assign,tx} )
            .await?;

        Ok(rx.await??)
    }

    pub async fn get(&self, key: ResourceKey ) -> Result<DataSet<BinSrc>, Fail> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send( ResourceStoreCommand::Get{key: key.clone(), tx } )
            .await?;
        Ok(rx.await??)
    }

    pub fn close(&self) {
        let tx = self.tx.clone();
        tokio::spawn( async move {
            tx
                .send(ResourceStoreCommand::Close)
                .await;
        });

    }
}

#[derive(strum_macros::Display)]
pub enum ResourceStoreCommand {
    Close,
    Save{assign: ResourceAssign<DataSet<BinSrc>>, tx: oneshot::Sender<Result<(),Error>>},
    Get{key:ResourceKey, tx: oneshot::Sender<Result<DataSet<BinSrc>,Error>>},
}

pub struct StateStoreFS {
    pub tx: mpsc::Sender<ResourceStoreCommand>,
    pub rx: mpsc::Receiver<ResourceStoreCommand>,
    pub skel: StarSkel
}

impl StateStoreFS {
    pub async fn new(skel: StarSkel) -> mpsc::Sender<ResourceStoreCommand> {
        let (tx, rx) = mpsc::channel(1024);
        let tx_clone = tx.clone();

        tokio::spawn( async move {
            Self{
                tx:  tx_clone,
                rx: rx,
                skel
            }.run().await;
        });

        tx
    }

    async fn run(mut self) -> Result<(), Error> {

        while let Option::Some(request) = self.rx.recv().await {
            if let ResourceStoreCommand::Close = request {
                break;
            } else {
                self.process(request).await;
            }
        }

        Ok(())
    }

    async fn save( &mut self, assign: ResourceAssign<DataSet<BinSrc>> ) -> Result<(),Error>{
        let key = assign.stub.key.to_string();

        let state_path= Path::from_str(format!("/stars/{}/states/{}",self.skel.info.key.to_string(), key).as_str())?;
        let machine_filesystem = self.skel.machine.machine_filesystem();
        let mut data_access = machine_filesystem.data_access();
        data_access.mkdir(&state_path).await?;

        let mut rxs = vec![];
        for (aspect, data) in assign.state_src {
            let state_aspect_file = Path::from_str(format!("/stars/{}/states/{}/{}",self.skel.info.key.to_string(),key.to_string(),aspect).as_str())?;
            let (tx,rx) = oneshot::channel();
            let bin_context = self.skel.machine.bin_context();
            data.mv( bin_context, state_aspect_file, tx ).await;
            rxs.push(rx);
        }

        for rx in rxs {
            rx.await?;
        }

        Ok(())
    }

    async fn get( &self, key: ResourceKey ) -> Result<DataSet<BinSrc>,Error>{
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

    async fn process(
        &mut self,
        command: ResourceStoreCommand,
    ) {
        match command {
            ResourceStoreCommand::Save{ assign, tx } => {
                tx.send(self.save(assign).await ).unwrap_or_default();
            }
            ResourceStoreCommand::Get { key, tx } => {
                tx.send(self.get(key).await ).unwrap_or_default();
            }
            ResourceStoreCommand::Close => {}
        }
    }


}
