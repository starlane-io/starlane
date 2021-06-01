use crate::resource::{ResourceAssign, ResourceType, Names, StateSrc};
use rusqlite::{Connection, Transaction,params};
use tokio::sync::{mpsc, oneshot};
use std::collections::HashSet;
use crate::error::Error;
use crate::message::Fail;
use std::iter::FromIterator;
use std::convert::TryInto;
use crate::resource::space::SpaceState;
use serde::{Deserialize, Serialize};
use crate::frame::ResourceHostAction;
use crate::core::Host;
use crate::resource;

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
}

pub struct SpaceHostAction {

    pub command: ResourceHostCommand,
    pub tx: oneshot::Sender<Result<ResourceHostResult,Fail>>
}

pub enum ResourceHostCommand {
    Close,
    Assign(ResourceAssign)
}


pub enum ResourceHostResult {
    Ok
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

                match assign.key.resource_type(){
                    ResourceType::Space => {
                        let trans = self.conn.transaction()?;
                        let space_key = assign.key.space()?;
                        let id = space_key.id();
                        let state_src :StateSrc<SpaceState> = assign.source.try_into()?;
                        let state = state_src.to_state()?;
                        let name = state.name();
                        let display = state.display();
println!("INSERT INTO spaces (id,name,display) VALUES ({},{},{})", id,name,display);
                        trans.execute("INSERT INTO spaces (id,name,display) VALUES (?1,?2,?3)", params![id,name,display])?;

                        trans.commit()?;
                        Ok(ResourceHostResult::Ok)
                    }
/*                    ResourceType::SubSpace => {}
                    ResourceType::User => {}

 */
                    resource_type => {

                        Err(Fail::WrongResourceType { expected: HashSet::from_iter(vec![ResourceType::Space,ResourceType::SubSpace,ResourceType::User]), received: resource_type })
                    }
                }

            }
        }
    }

    pub fn setup(&mut self)->Result<(),Error>
    {
       let spaces = r#"
       CREATE TABLE IF NOT EXISTS spaces (
	      id INTEGER PRIMARY KEY AUTOINCREMENT,
	      name TEXT NOT NULL,
	      display TEXT NOT NULL
        )"#;

       let users= r#"
       CREATE TABLE IF NOT EXISTS users (
	      id INTEGER PRIMARY KEY AUTOINCREMENT,
	      space_id INTEGER NOT NULL,
	      email TEXT NOT NULL,
          UNIQUE(space_id,email),
          FOREIGN KEY (space_id) REFERENCES spaces (id)
        )"#;

       let sub_spaces= r#"
       CREATE TABLE IF NOT EXISTS sub_spaces (
	      id INTEGER PRIMARY KEY AUTOINCREMENT,
	      space_id INTEGER NOT NULL,
	      name TEXT NOT NULL,
	      display TEXT,
          UNIQUE(space_id,name),
          FOREIGN KEY (space_id) REFERENCES spaces (id)
        )"#;

        let transaction = self.conn.transaction()?;
        transaction.execute(spaces, [])?;
        transaction.execute(users, [])?;
        transaction.execute(sub_spaces, [])?;
        transaction.commit()?;

        Ok(())
    }
}