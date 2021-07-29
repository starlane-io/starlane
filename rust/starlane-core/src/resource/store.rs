
use std::str::FromStr;
use std::sync::Arc;

use rusqlite::{Connection, params, Row};
use rusqlite::types::ValueRef;
use tokio::sync::{mpsc, oneshot};

use starlane_resources::{ResourceIdentifier, ResourceStatePersistenceManager};

use crate::app::ConfigSrc;
use crate::error::Error;

use crate::message::Fail;
use crate::resource::{LocalStateSetSrc, Resource, ResourceAddress, ResourceArchetype, ResourceAssign, ResourceCreate, ResourceKey, ResourceKind, Specific};
use std::convert::TryInto;
use crate::data::{DataSet, BinSrc};

#[derive(Clone,Debug)]
pub struct ResourceStore {
    tx: mpsc::Sender<ResourceStoreAction>,
}

impl ResourceStore {
    pub async fn new() -> Self {
        ResourceStore {
            tx: ResourceStoreFS::new().await,
        }
    }

    pub async fn put(
        &self,
        assign: ResourceAssign<DataSet<BinSrc>>,
    ) -> Result<Resource, Fail> {
        let (tx, rx) = oneshot::channel();

        self.tx
            .send(ResourceStoreAction {
                command: ResourceStoreCommand::Put(assign),
                tx: tx,
            })
            .await?;

        match rx.await?? {
            ResourceStoreResult::Resource(resource) => {
                resource.ok_or(Fail::Error("option returned None".into()))
            }
            _ => Err(Fail::Error(
                "unexpected response from host registry sql".into(),
            )),
        }
    }

    pub async fn get(&self, key: ResourceKey ) -> Result<Option<Resource>, Fail> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(ResourceStoreAction {
                command: ResourceStoreCommand::Get(key.clone()),
                tx: tx,
            })
            .await?;
        let result = rx.await??;
        match result {
            ResourceStoreResult::Resource(resource) => Ok(resource),
            what => Err(Fail::Unexpected{ expected: "Resource()".to_string(), received: what.to_string()}),
        }
    }

    pub fn close(&self) {
        let tx = self.tx.clone();
        tokio::spawn( async move {
            tx
                .send(ResourceStoreAction {
                    command: ResourceStoreCommand::Close,
                    tx: oneshot::channel().0
                })
                .await;
        });

    }
}

pub struct ResourceStoreAction {
    pub command: ResourceStoreCommand,
    pub tx: oneshot::Sender<Result<ResourceStoreResult, Fail>>,
}

#[derive(strum_macros::Display)]
pub enum ResourceStoreCommand {
    Close,
    Put(ResourceAssign<DataSet<BinSrc>>),
    Get(ResourceKey),
}

pub enum ResourceStoreResult {
    Ok,
    Resource(Option<Resource>),
}

impl ToString for ResourceStoreResult{
    fn to_string(&self) -> String {
        match self {
            ResourceStoreResult::Ok => "ResourceStoreResult::Ok".to_string(),
            ResourceStoreResult::Resource(_) => "ResourceStoreResult::Resource(_)".to_string()
        }
    }
}

pub struct ResourceStoreFS {
    pub conn: Connection,
    pub tx: mpsc::Sender<ResourceStoreAction>,
    pub rx: mpsc::Receiver<ResourceStoreAction>,
}

impl ResourceStoreFS {
    pub async fn new() -> mpsc::Sender<ResourceStoreAction> {
        let (tx, rx) = mpsc::channel(1024);

        let tx_clone = tx.clone();
        tokio::spawn(async move {
            let conn = Connection::open_in_memory();
            if conn.is_ok() {
                let mut db = ResourceStoreFS {
                    conn: conn.unwrap(),
                    tx: tx_clone,
                    rx: rx,
                };
                match db.run().await {
                    Ok(_) => {}
                    Err(err) => {
                        eprintln!("experienced fatal error in sql db: {}", err);
                    }
                }
            }
        });
        tx
    }

    async fn run(&mut self) -> Result<(), Error> {
        match self.setup() {
            Ok(_) => {}
            Err(err) => {
                eprintln!("error setting up db: {}", err);
                return Err(err);
            }
        };

        while let Option::Some(request) = self.rx.recv().await {
            if let ResourceStoreCommand::Close = request.command {
                request.tx.send(Ok(ResourceStoreResult::Ok));
                break;
            } else {
                request.tx.send(self.process(request.command).await);
            }
        }

        Ok(())
    }

    async fn process(
        &mut self,
        command: ResourceStoreCommand,
    ) -> Result<ResourceStoreResult, Fail> {
        match command {
            ResourceStoreCommand::Close => Ok(ResourceStoreResult::Ok),
            ResourceStoreCommand::Put(assign) => {
                let key = assign.stub.key.bin()?;
                let address = assign.stub.address.to_string();
                let specific = match &assign.stub.archetype.specific {
                    None => Option::None,
                    Some(specific) => Option::Some(specific.to_string()),
                };
                let config_src = match &assign.stub.archetype.config {
                    None => Option::None,
                    Some(config_src) => Option::Some(config_src.to_string()),
                };

                unimplemented!();
                /*
                let state = match assign
                    .stub
                    .archetype
                    .kind
                    .resource_type()
                    .state_persistence()
                {
                    ResourceStatePersistenceManager::Store => {
                        let state_src: DataSetBlob = assign.state_src.clone().try_into()?;
                        state_src.bin()?
                    }
                    _ => {
                        DataSetBlob::new().bin()?
                    }
                };

//                self.conn.execute("INSERT INTO resources (key,address,state_src,kind,specific,config_src) VALUES (?1,?2,?3,?4,?5,?6)", params![key,address,state,assign.stub.archetype.kind.to_string(),specific,config_src])?;

                 */

                let resource = Resource::new(
                    assign.stub.key,
                    assign.stub.address,
                    assign.stub.archetype,
                    assign.state_src
                );

                Ok(ResourceStoreResult::Resource(Option::Some(resource)))
            }
            ResourceStoreCommand::Get(identifier) => {
                unimplemented!()
            }
        }
    }

    pub fn setup(&mut self) -> Result<(), Error> {

        Ok(())
    }
}
