use crate::resource::{ResourceAssign, ResourceType, Names, Resource, ResourceAddress, ResourceStateSrc, AssignResourceStateSrc};
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
  tx: mpsc::Sender<SpaceHostAction>
}

impl SpaceHost {
    pub async fn new()->Self{
        SpaceHost {
            tx: SpaceHostSqLite::new().await
        }
    }
}
#[async_trait]
impl Host for SpaceHost {


    async fn assign(&self, assign: ResourceAssign) -> Result<(), Fail> {
println!("assignging resource:{} ", assign.archetype.kind );
        let (tx,rx) = oneshot::channel();
        self.tx.send( SpaceHostAction{
            command: ResourceHostCommand::Assign(assign.clone()),
            tx: tx
        }).await?;
        rx.await?;
println!("AsSiGnEd:{} ", assign.archetype.kind );
        Ok(())
    }

    async fn get(&self, key: ResourceKey) -> Result<Option<Resource>, Fail> {
        let (tx,rx) = oneshot::channel();
        self.tx.send( SpaceHostAction{
            command: ResourceHostCommand::Get(key.clone()),
            tx: tx
        }).await?;
        let result = rx.await??;
        match result {
            ResourceHostResult::Resource(resource) => {
                Ok(resource)
            }
            _ => Err(Fail::Unexpected)
        }
    }
}

pub struct SpaceHostAction {

    pub command: ResourceHostCommand,
    pub tx: oneshot::Sender<Result<ResourceHostResult,Fail>>
}

pub enum ResourceHostCommand {
    Close,
    Assign(ResourceAssign),
    Get(ResourceKey)
}


pub enum ResourceHostResult {
    Ok,
    Resource(Option<Resource>)
}


pub struct SpaceHostSqLite {
    pub conn: Connection,
    pub tx: mpsc::Sender<SpaceHostAction>,
    pub rx: mpsc::Receiver<SpaceHostAction>,
    pub accepted: Option<HashSet<ResourceType>>
}

impl SpaceHostSqLite {
    pub async fn new() -> mpsc::Sender<SpaceHostAction>
    {
        let (tx, rx) = mpsc::channel(1024 );

        let tx_clone = tx.clone();
        tokio::spawn(async move {

            let conn = Connection::open_in_memory();
            if conn.is_ok()
            {
                let mut db = SpaceHostSqLite {
                    conn: conn.unwrap(),
                    tx: tx_clone,
                    rx: rx,
                    accepted: Option::None
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
            if let ResourceHostCommand::Close = request.command
            {
                request.tx.send(Ok(ResourceHostResult::Ok) );
                break;
            }
            else {
                request.tx.send(self.process(request.command));
            }
        }

        Ok(())
    }

    fn process(&mut self, command: ResourceHostCommand) -> Result<ResourceHostResult, Fail> {
        match command
        {
            ResourceHostCommand::Close => {
                Ok(ResourceHostResult::Ok)
            }
            ResourceHostCommand::Assign(assign) => {
                let trans = self.conn.transaction()?;
                let key = assign.key.bin()?;
                let address = assign.address.to_string();
                let state_src = assign.state_src.to_resource_state_src(assign.key.resource_type() )?;
println!("state_src...{}",state_src.resource_type().to_string());
                let state: Arc<Vec<u8>> = state_src.try_into()?;

                trans.execute("INSERT INTO resources (key,address,state) VALUES (?1,?2,?3)", params![key,address,*state])?;
                trans.commit()?;
                Ok(ResourceHostResult::Ok)
            }
            ResourceHostCommand::Get(key) => {
println!("Get Resource...");
                let key_bin = key.bin()?;
                let resource = self.conn.query_row("SELECT address,state FROM resources WHERE key=?1", params![key_bin], |row| {
println!("ROW: ....");
                    let address: String = row.get(0)?;
println!("x: ....");
                    let address= ResourceAddress::from_str(address.as_str())?;
println!("y: ....");
                    let state: Vec<u8> = row.get(1)?;
println!("z: ....");
                    let state= ResourceStateSrc::try_from(state);
                    match &state {
                        Ok(x) => {}
                        Err(err) => { println!("error: {}",err)}
                    }
                    let state = state?;
println!("w: ....");
                    Ok(Resource::new(key,address,state))
                });

                match resource {
                    Ok(resource) => {
                        Ok(ResourceHostResult::Resource(Option::Some(resource)))
                    }
                    Err(err) => {

println!("SQL ERR.......");
                        match err {
                        rusqlite::Error::QueryReturnedNoRows => Ok(ResourceHostResult::Resource(Option::None)),
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
	      state BLOB NOT NULL
        )"#;

        let transaction = self.conn.transaction()?;
        transaction.execute(resources, [])?;
        transaction.commit()?;

        Ok(())
    }
}