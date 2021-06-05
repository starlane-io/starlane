use crate::resource::{ResourceAssign, ResourceType, Names, Resource, ResourceAddress, ResourceStateSrc, AssignResourceStateSrc, DataTransfer, MemoryDataTransfer, LocalDataSrc};
use rusqlite::{Connection, Transaction,params};
use tokio::sync::{mpsc, oneshot};
use std::collections::HashSet;
use crate::error::Error;
use crate::message::Fail;
use std::iter::FromIterator;
use std::convert::{TryInto, TryFrom};
use crate::resource::space::{SpaceState, Space};
use serde::{Deserialize, Serialize};
use crate::frame::ResourceHostAction;
use crate::core::Host;
use crate::resource;
use crate::resource::user::UserState;
use crate::keys::{ResourceKey, SpaceId};
use std::str::FromStr;
use std::sync::Arc;

pub struct SpaceHost {
  tx: mpsc::Sender<ResourceStoreAction>
}

impl SpaceHost {
    pub async fn new()->Self{
        SpaceHost {
            tx: ResourceStoreSqlLite::new().await
        }
    }
}
#[async_trait]
impl Host for SpaceHost {


    async fn assign(&self, assign: ResourceAssign<AssignResourceStateSrc>) -> Result<(), Fail> {
        let (tx,rx) = oneshot::channel();

        // if there is Initialization to do for assignment THIS is where we do it
        let data = match assign.state_src{
            AssignResourceStateSrc::Direct(data) => data
        };
        let data_transfer:Arc<dyn DataTransfer> = Arc::new(MemoryDataTransfer::new(data));

        let assign = ResourceAssign{
            key: assign.key,
            address: assign.address,
            archetype: assign.archetype,
            state_src: data_transfer
        };

        self.tx.send( ResourceStoreAction {
            command: ResourceStoreCommand::Put(assign),
            tx: tx
        }).await?;
        rx.await?;
        Ok(())
    }

    async fn get(&self, key: ResourceKey) -> Result<Option<Resource>, Fail> {
        let (tx,rx) = oneshot::channel();
        self.tx.send( ResourceStoreAction {
            command: ResourceStoreCommand::Get(key.clone()),
            tx: tx
        }).await?;
        let result = rx.await??;
        match result {
            ResourceStoreResult::Resource(resource) => {
                Ok(resource)
            }
            _ => Err(Fail::Unexpected)
        }
    }
}

pub struct ResourceStoreAction {

    pub command: ResourceStoreCommand,
    pub tx: oneshot::Sender<Result<ResourceStoreResult,Fail>>
}

pub enum ResourceStoreCommand {
    Close,
    Put(ResourceAssign<Arc<dyn DataTransfer>>),
    Get(ResourceKey)
}


pub enum ResourceStoreResult {
    Ok,
    Resource(Option<Resource>)
}


pub struct ResourceStoreSqlLite {
    pub conn: Connection,
    pub tx: mpsc::Sender<ResourceStoreAction>,
    pub rx: mpsc::Receiver<ResourceStoreAction>,
}

impl ResourceStoreSqlLite {
    pub async fn new() -> mpsc::Sender<ResourceStoreAction>
    {
        let (tx, rx) = mpsc::channel(1024 );

        let tx_clone = tx.clone();
        tokio::spawn(async move {

            let conn = Connection::open_in_memory();
            if conn.is_ok()
            {
                let mut db = ResourceStoreSqlLite {
                    conn: conn.unwrap(),
                    tx: tx_clone,
                    rx: rx,
                };
                db.run().await.unwrap();
            }
        });
        tx
    }

    async fn run(&mut self) -> Result<(), Error>
    {
        match self.setup()
        {
            Ok(_) => {}
            Err(err) => {
                eprintln!("error setting up db: {}", err );
                return Err(err);
            }
        };

        while let Option::Some(request) = self.rx.recv().await {
            if let ResourceStoreCommand::Close = request.command
            {
                request.tx.send(Ok(ResourceStoreResult::Ok) );
                break;
            }
            else {
                request.tx.send(self.process(request.command));
            }
        }

        Ok(())
    }

    fn process(&mut self, command: ResourceStoreCommand) -> Result<ResourceStoreResult, Fail> {
        match command
        {
            ResourceStoreCommand::Close => {
                Ok(ResourceStoreResult::Ok)
            }
            ResourceStoreCommand::Put(assign) => {
                let trans = self.conn.transaction()?;
                let key = assign.key.bin()?;
                let address = assign.address.to_string();

                let state = assign.state_src.get()?;

                trans.execute("INSERT INTO resources (key,address,state) VALUES (?1,?2,?3)", params![key,address,*state])?;
                trans.commit()?;
                Ok(ResourceStoreResult::Ok)
            }
            ResourceStoreCommand::Get(key) => {
                let key_bin = key.bin()?;
                let resource = self.conn.query_row("SELECT address,state FROM resources WHERE key=?1", params![key_bin], |row| {
                    let address: String = row.get(0)?;
                    let address= ResourceAddress::from_str(address.as_str())?;
                    let state: Vec<u8> = row.get(1)?;
                    let state= Arc::new(MemoryDataTransfer::new(Arc::new(state)));
                    Ok(Resource::new(key,address,state))
                });

                match resource {
                    Ok(resource) => {
                        Ok(ResourceStoreResult::Resource(Option::Some(resource)))
                    }
                    Err(err) => {

                        match err {
                        rusqlite::Error::QueryReturnedNoRows => Ok(ResourceStoreResult::Resource(Option::None)),
                        _ => Err(err.into())
                    }}
                }

            }
        }
    }

    pub fn setup(&mut self)->Result<(),Error>
    {
       let resources= r#"
       CREATE TABLE IF NOT EXISTS resources(
	      key BLOB PRIMARY KEY,
	      address TEXT NOT NULL,
	      state BLOB NOT NULL,
	      UNIQUE(address)
        )"#;

        let transaction = self.conn.transaction()?;
        transaction.execute(resources, [])?;
        transaction.commit()?;

        Ok(())
    }
}