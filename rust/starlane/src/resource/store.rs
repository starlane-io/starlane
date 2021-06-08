use std::convert::TryInto;
use std::str::FromStr;
use std::sync::Arc;

use rusqlite::{Connection, params, Transaction};
use rusqlite::types::ValueRef;
use tokio::sync::{mpsc, oneshot};

use crate::app::ConfigSrc;
use crate::error::Error;
use crate::file::FileAccess;
use crate::keys::ResourceKey;
use crate::message::Fail;
use crate::names::Specific;
use crate::resource::{DataTransfer, FileDataTransfer, LocalDataSrc, MemoryDataTransfer, Resource, ResourceAddress, ResourceArchetype, ResourceAssign, ResourceKind, ResourceStatePersistenceManager};

#[derive(Clone)]
pub struct ResourceStore{
   tx: mpsc::Sender<ResourceStoreAction>
}

impl ResourceStore{

    pub async fn new()->Self {
        ResourceStore{
          tx: ResourceStoreSqlLite::new().await
        }
    }

    pub async fn put(&self, assign: ResourceAssign<Arc<dyn DataTransfer>>) -> Result<Resource, Fail> {
        let (tx,rx) = oneshot::channel();

        self.tx.send( ResourceStoreAction {
            command: ResourceStoreCommand::Put(assign),
            tx: tx
        }).await?;

        match rx.await??{
            ResourceStoreResult::Resource(resource) => {
                println!("IS SOME? {}",resource.is_some());
                resource.ok_or(Fail::Error("option returned None".into()))
            }
            _ => Err(Fail::Error("unexpected response from host registry sql".into()))
        }

    }

    pub async fn get(&self, key: ResourceKey) -> Result<Option<Resource>, Fail> {
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
                    rx: rx
                };
                match db.run().await
                {
                    Ok(_) => {}
                    Err(err) => {
                        eprintln!("experienced fatal error in sql db: {}", err);
                    }
                }
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
                request.tx.send(self.process(request.command).await );
            }
        }

        Ok(())
    }

    async fn process(&mut self, command: ResourceStoreCommand) -> Result<ResourceStoreResult, Fail> {
        match command
        {
            ResourceStoreCommand::Close => {
                Ok(ResourceStoreResult::Ok)
            }
            ResourceStoreCommand::Put(assign) => {
println!("PUT!");
                let key = assign.key.bin()?;
                let address = assign.address.to_string();
                let specific = match &assign.archetype.specific{
                    None => Option::None,
                    Some(specific) => Option::Some(specific.to())
                };
                let config_src = match &assign.archetype.config {
                    None => Option::None,
                    Some(config_src) => Option::Some(config_src.to_string())
                };

                let state = match assign.archetype.kind.resource_type().state_persistence(){
                    ResourceStatePersistenceManager::Store => {Option::Some(assign.state_src.get().await?)}
                    _ => Option::None
                };

                self.conn.execute("INSERT INTO resources (key,address,state_src,kind,specific,config_src) VALUES (?1,?2,?3,?4,?5,?6)", params![key,address,state,assign.archetype.kind.to_string(),specific,config_src])?;

                let resource = Resource::new(assign.key,assign.address, assign.archetype, assign.state_src );

println!("returning Resource....");
                Ok(ResourceStoreResult::Resource(Option::Some(resource)))
            }
            ResourceStoreCommand::Get(key) => {
                let key_bin = key.bin()?;
                let resource = self.conn.query_row("SELECT address,state_src,kind,specific,config_src FROM resources WHERE key=?1", params![key_bin], |row| {
                    let address: String = row.get(0)?;
                    let address= ResourceAddress::from_str(address.as_str())?;

                    let state= if let ValueRef::Null = row.get_ref(4)? {
                        Option::None
                    } else {
                        let state: Vec<u8>= row.get(1)?;
                        Option::Some(state)
                    };

                    let kind: String = row.get(2)?;
                    let kind = ResourceKind::from_str( kind.as_str() )?;
                    let specific= if let ValueRef::Null = row.get_ref(3)? {
                        Option::None
                    } else {
                        let specific: String = row.get(3)?;
                        let specific= Specific::from_str(specific.as_str())?;
                        Option::Some(specific)
                    };

                    let config_src= if let ValueRef::Null = row.get_ref(4)? {
                        Option::None
                    } else {
                        let config_src: String = row.get(4)?;
                        let config_src = ConfigSrc::from_str(config_src.as_str())?;
                        Option::Some(config_src)
                    };

                    let state: Arc<dyn DataTransfer> = match state {
                        None => {Arc::new(MemoryDataTransfer::none())}
                        Some(state) => {Arc::new(MemoryDataTransfer::new(Arc::new(state)))}
                    };

                    let archetype = ResourceArchetype{
                        kind: kind,
                        specific: specific,
                        config: config_src
                    };

                    Ok(Resource::new(key,address, archetype, state))
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
	      state_src BLOB,
	      kind TEXT NOT NULL,
	      specific TEXT,
	      config_src TEXT,
	      UNIQUE(address)
        )"#;

        let transaction = self.conn.transaction()?;
        transaction.execute(resources, [])?;
        transaction.commit()?;

        Ok(())
    }
}
