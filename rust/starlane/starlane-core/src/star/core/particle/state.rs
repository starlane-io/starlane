use std::convert::TryInto;
use std::str::FromStr;
use std::sync::Arc;

use tokio::sync::{mpsc, oneshot};

use mesh_portal::version::latest::id::Point;
use mesh_portal::version::latest::path::Path;
use mesh_portal::version::latest::payload::Substance;

use crate::error::Error;
use crate::fail::Fail;
use crate::file_access::FileAccess;
use crate::star::StarSkel;
use crate::starlane::files::MachineFileSystem;

#[derive(Clone, Debug)]
pub struct StateStore {
    tx: mpsc::Sender<ResourceStoreCommand>,
}

impl StateStore {
    pub fn new(skel: StarSkel) -> Self {
        StateStore {
            tx: StateStoreFS::new(skel),
        }
    }

    pub async fn has(&self, address: Point) -> Result<bool, Error> {
        let (tx, rx) = oneshot::channel();

        self.tx
            .send(ResourceStoreCommand::Has { address, tx })
            .await?;

        rx.await?
    }

    pub async fn put(&self, key: Point, state: Substance) -> Result<(), Error> {
        let (tx, rx) = oneshot::channel();

        self.tx
            .send(ResourceStoreCommand::Save {
                address: key,
                state,
                tx,
            })
            .await?;
        rx.await?
    }

    pub async fn get(&self, address: Point) -> Result<Substance, Error> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(ResourceStoreCommand::Get { address, tx })
            .await?;
        rx.await?
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
        address: Point,
        state: Substance,
        tx: oneshot::Sender<Result<(), Error>>,
    },
    Get {
        address: Point,
        tx: oneshot::Sender<Result<Substance, Error>>,
    },
    Has {
        address: Point,
        tx: oneshot::Sender<Result<bool, Error>>,
    },
}

pub struct StateStoreFS {
    pub tx: mpsc::Sender<ResourceStoreCommand>,
    pub rx: mpsc::Receiver<ResourceStoreCommand>,
    pub skel: StarSkel,
}

impl StateStoreFS {
    pub fn new(skel: StarSkel) -> mpsc::Sender<ResourceStoreCommand> {
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

    async fn save(&mut self, address: Point, state: Substance) -> Result<(), Error> {
        let parent_path = Path::from_str(
            format!(
                "/stars/{}/states/{}",
                self.skel.info.key.to_string(),
                address.to_safe_filename()
            )
            .as_str(),
        )?;
        let state_path = Path::from_str(
            format!(
                "/stars/{}/states/{}/payload.bin",
                self.skel.info.key.to_string(),
                address.to_safe_filename()
            )
            .as_str(),
        )?;
        let machine_filesystem = self.skel.machine.machine_filesystem();
        let mut data_access = machine_filesystem.data_access();
        data_access.mkdir(&parent_path).await;

        let data_access = self.skel.machine.machine_filesystem().data_access();
        data_access
            .write(&state_path, Arc::new(bincode::serialize(&state)?))
            .await?;

        Ok(())
    }

    async fn get(&self, address: Point) -> Result<Substance, Error> {
        let machine_filesystem = self.skel.machine.machine_filesystem();
        let mut data_access = machine_filesystem.data_access();

        let state_path = Path::from_str(
            format!(
                "/stars/{}/states/{}/payload.bin",
                self.skel.info.key.to_string(),
                address.to_safe_filename()
            )
            .as_str(),
        )?;

        let bin = data_access.read(&state_path).await?;
        let payload: Substance = bincode::deserialize(bin.as_slice())?;
        Ok(payload)
    }

    async fn has(&self, address: Point) -> Result<bool, Error> {
        let state_path = Path::from_str(
            format!(
                "/stars/{}/states/{}/payload.bin",
                self.skel.info.key.to_string(),
                address.to_safe_filename()
            )
            .as_str(),
        )?;

        let machine_filesystem = self.skel.machine.machine_filesystem();
        let data_access = machine_filesystem.data_access();
        Ok(data_access.exists(&state_path).await?)
    }

    async fn process(&mut self, command: ResourceStoreCommand) {
        match command {
            ResourceStoreCommand::Save {
                address: key,
                state,
                tx,
            } => {
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
