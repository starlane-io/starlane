use std::convert::TryInto;
use std::str::FromStr;
use std::sync::Arc;

use rusqlite::{Connection, params, Row};
use rusqlite::types::ValueRef;
use tokio::sync::{mpsc, oneshot};

use crate::error::Error;
use crate::file_access::FileAccess;
use crate::resource::{
     Kind, Specific,
};
use crate::star::StarSkel;
use crate::starlane::files::MachineFileSystem;
use crate::mesh::serde::id::Address;
use crate::mesh::serde::payload::Payload;
use mesh_portal_parse::path::Path;
use crate::fail::Fail;

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

    pub async fn has(&self, address: Address) -> Result<bool, Fail> {
        let (tx, rx) = oneshot::channel();

        self.tx.send(ResourceStoreCommand::Has { address, tx }).await?;

        Ok(rx.await?)
    }

    pub async fn put(
        &self,
        key: Address,
        state : Payload,
    ) -> Result<(), Fail> {
        let (tx, rx) = oneshot::channel();

        self.tx
            .send(ResourceStoreCommand::Save { address: key, state, tx })
            .await?;
        rx.await??;
        Ok(())
    }

    pub async fn get(&self, address: Address) -> Result<Payload, Fail> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(ResourceStoreCommand::Get {
                address,
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
        address: Address,
        state: Payload,
        tx: oneshot::Sender<Result<(), Fail>>,
    },
    Get {
        address: Address,
        tx: oneshot::Sender<Result<Payload, Fail>>,
    },
    Has {
        address: Address,
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
        state: Payload,
    ) -> Result<Payload, Error> {
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

            let data_access = self.skel.machine.machine_filesystem().data_access();
            data_access.write(&state_aspect_file,bin).await?;
        }

/*        for rx in rxs {
            rx.await?;
        }

 */

        Ok(state)
    }

    async fn get(&self, address: Address ) -> Result<Payload, Error> {
        unimplemented!();
        /*
        let machine_filesystem = self.skel.machine.machine_filesystem();
        let mut data_access = machine_filesystem.data_access();

        let state_path = Path::from_str(
            format!("/stars/{}/states/{}", self.skel.info.key.to_string(), address.to_string()).as_str(),
        )?;
        let mut dataset = DataSet::new();
        for aspect in data_access.list(&state_path).await? {
            let state_aspect_file = Path::from_str(
                format!(
                    "/stars/{}/states/{}/{}",
                    self.skel.info.key.to_string(),
                    address.to_string(),
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
//        Ok()

         */
    }

    async fn has(&self, address: Address ) -> bool {
        if let Ok(Some(_)) = self.get(address).await {
            true
        } else {
            false
        }
    }

    async fn process(&mut self, command: ResourceStoreCommand) {
        match command {
            ResourceStoreCommand::Save { address: key, state, tx } => {
                tx.send(self.save(key, state).await).unwrap_or_default();
            }
            ResourceStoreCommand::Get { address: key, tx } => {
                tx.send(self.get(key).await).unwrap_or_default();
            }
            ResourceStoreCommand::Has { address: key, tx } => {
                tx.send(self.has(key).await).unwrap_or_default();
            }
            ResourceStoreCommand::Close => {}
        }
    }
}
