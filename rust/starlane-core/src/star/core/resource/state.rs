use std::convert::TryInto;
use std::str::FromStr;
use std::sync::Arc;

use rusqlite::{Connection, params, Row};
use rusqlite::types::ValueRef;
use tokio::sync::{mpsc, oneshot};

use starlane_resources::{LocalStateSetSrc, Resource, ResourceArchetype, ResourceAssign, ResourceCreate, ResourceIdentifier, ResourceStatePersistenceManager};
use starlane_resources::data::{BinSrc, DataSet};
use starlane_resources::message::Fail;

use starlane_resources::ConfigSrc;
use crate::error::Error;
use crate::file_access::FileAccess;
use crate::resource::{
    Path, ResourceAddress,
    ResourceKey, ResourceKind, Specific,
};
use crate::star::StarSkel;
use crate::starlane::files::MachineFileSystem;

#[derive(Clone, Debug)]
pub struct StateStore {
    tx: mpsc::Sender<ResourceStoreCommand>,
}

impl StateStore {
    pub async fn new(skel: StarSkel) -> Self {
        StateStore {
            tx: StateStoreFS::new(skel).await,
        }
    }

    pub async fn has(&self, key: ResourceKey) -> Result<bool, Error> {
        let (tx, rx) = oneshot::channel();

        self.tx.send(ResourceStoreCommand::Has { key, tx }).await?;

        Ok(rx.await?)
    }

    pub async fn put(
        &self,
        key: ResourceKey,
        state : DataSet<BinSrc>,
    ) -> Result<DataSet<BinSrc>, Error> {
        let (tx, rx) = oneshot::channel();

        self.tx
            .send(ResourceStoreCommand::Save { key, state, tx })
            .await?;

        Ok(rx.await??)
    }

    pub async fn get(&self, key: ResourceKey) -> Result<Option<DataSet<BinSrc>>, Error> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(ResourceStoreCommand::Get {
                key: key.clone(),
                tx,
            })
            .await?;
        Ok(rx.await??)
    }

    pub fn close(&self) {
        let tx = self.tx.clone();
        tokio::spawn(async move {
            tx.send(ResourceStoreCommand::Close).await;
        });
    }
}

#[derive(strum_macros::Display)]
pub enum ResourceStoreCommand {
    Close,
    Save {
        key: ResourceKey,
        state: DataSet<BinSrc>,
        tx: oneshot::Sender<Result<DataSet<BinSrc>, Error>>,
    },
    Get {
        key: ResourceKey,
        tx: oneshot::Sender<Result<Option<DataSet<BinSrc>>, Error>>,
    },
    Has {
        key: ResourceKey,
        tx: oneshot::Sender<bool>,
    },
}

pub struct StateStoreFS {
    pub tx: mpsc::Sender<ResourceStoreCommand>,
    pub rx: mpsc::Receiver<ResourceStoreCommand>,
    pub skel: StarSkel,
}

impl StateStoreFS {
    pub async fn new(skel: StarSkel) -> mpsc::Sender<ResourceStoreCommand> {
        let (tx, rx) = mpsc::channel(1024);
        let tx_clone = tx.clone();

        tokio::spawn(async move {
            Self {
                tx: tx_clone,
                rx: rx,
                skel,
            }
            .run()
            .await;
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

    async fn save(
        &mut self,
        key: ResourceKey,
        state: DataSet<BinSrc>,
    ) -> Result<DataSet<BinSrc>, Error> {
        let key = key.to_snake_case();

        let state_path = Path::from_str(
            format!("/stars/{}/states/{}", self.skel.info.key.to_string(), key).as_str(),
        )?;
        let machine_filesystem = self.skel.machine.machine_filesystem();
        let mut data_access = machine_filesystem.data_access();
        data_access.mkdir(&state_path).await?;

//        let mut rxs = vec![];
        for (aspect, data) in &state {
            let state_aspect_file = Path::from_str(
                format!(
                    "/stars/{}/states/{}/{}",
                    self.skel.info.key.to_string(),
                    key,
                    aspect
                )
                .as_str(),
            )?;
            let bin_context = self.skel.machine.bin_context();
            let bin = data.to_bin(bin_context)?;

            let data_access = self.skel.machine.machine_filesystem().data_access();
            data_access.write(&state_aspect_file,bin).await?;
        }

/*        for rx in rxs {
            rx.await?;
        }

 */

        Ok(state)
    }

    async fn get(&self, key: ResourceKey) -> Result<Option<DataSet<BinSrc>>, Error> {
        let key = key.to_snake_case();
        let machine_filesystem = self.skel.machine.machine_filesystem();
        let mut data_access = machine_filesystem.data_access();

        let state_path = Path::from_str(
            format!("/stars/{}/states/{}", self.skel.info.key.to_string(), key).as_str(),
        )?;
        let mut dataset = DataSet::new();
        for aspect in data_access.list(&state_path).await? {
            let state_aspect_file = Path::from_str(
                format!(
                    "/stars/{}/states/{}/{}",
                    self.skel.info.key.to_string(),
                    key,
                    aspect
                        .last_segment()
                        .ok_or("expected final segment from list")?
                )
                .as_str(),
            )?;

            let bin = data_access.read(&state_aspect_file).await?;
            let bin_src = BinSrc::Memory(bin);
            dataset.insert(
                aspect
                    .last_segment()
                    .ok_or("expected final segment from list")?,
                bin_src,
            );
        }
        Ok(Option::Some(dataset))
    }

    async fn has(&self, key: ResourceKey) -> bool {
        if let Ok(Some(_)) = self.get(key).await {
            true
        } else {
            false
        }
    }

    async fn process(&mut self, command: ResourceStoreCommand) {
        match command {
            ResourceStoreCommand::Save { key, state, tx } => {
                tx.send(self.save(key, state).await).unwrap_or_default();
            }
            ResourceStoreCommand::Get { key, tx } => {
                tx.send(self.get(key).await).unwrap_or_default();
            }
            ResourceStoreCommand::Has { key, tx } => {
                tx.send(self.has(key).await).unwrap_or_default();
            }
            ResourceStoreCommand::Close => {}
        }
    }
}
