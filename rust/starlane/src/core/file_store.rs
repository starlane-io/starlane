use crate::core::space::{ResourceStoreAction, ResourceStoreCommand, ResourceStoreResult};
use rusqlite::Connection;
use tokio::sync::mpsc;
use crate::error::Error;
use crate::message::Fail;
use crate::resource::{ResourceStateSrc, ResourceAddress, Resource, ResourceAssign, AssignResourceStateSrc};
use std::convert::{TryInto, TryFrom};
use std::sync::Arc;
use std::str::FromStr;
use crate::core::Host;
use crate::keys::ResourceKey;

pub struct FileStoreHost {
    tx: mpsc::Sender<ResourceStoreAction>
}

impl FileStoreHost {
    pub async fn new()->Self{
        FileStoreHost {
            tx: FileRegistry::new().await
        }
    }
}
#[async_trait]
impl Host for FileStoreHost {
    async fn assign(&self, assign: ResourceAssign<AssignResourceStateSrc>) -> Result<(), Fail> {
        todo!()
    }

    async fn get(&self, key: ResourceKey) -> Result<Option<Resource>, Fail> {
        todo!()
    }
}

pub struct FileRegistry {
    pub conn: Connection,
    pub tx: mpsc::Sender<ResourceStoreAction>,
    pub rx: mpsc::Receiver<ResourceStoreAction>,
}

impl FileRegistry {
    pub async fn new() -> mpsc::Sender<ResourceStoreAction>
    {
        let (tx, rx) = mpsc::channel(1024 );

        let tx_clone = tx.clone();
        tokio::spawn(async move {

            let conn = Connection::open_in_memory();
            if conn.is_ok()
            {
                let mut db = FileRegistry {
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

                Ok(ResourceStoreResult::Ok)
            }
            ResourceStoreCommand::Get(key) => {

                Ok(ResourceStoreResult::Ok)
            }
        }
    }

    pub fn setup(&mut self)->Result<(),Error>
    {
        let file_systems = r#"
       CREATE TABLE IF NOT EXISTS file_systems(
	      key BLOB PRIMARY KEY,
	      address TEXT NOT NULL,
	      UNIQUE(address)
        )"#;

        let files = r#"
       CREATE TABLE IF NOT EXISTS files (
	      key BLOB PRIMARY KEY,
	      address TEXT NOT NULL,
	      file_system_key BLOB NOT NULL,
	      FOREIGN KEY (file_system_key) REFERENCES file_systems(key),
	      UNIQUE(address)
        )"#;

        let transaction = self.conn.transaction()?;
        transaction.execute(file_systems, [])?;
        transaction.execute(files, [])?;
        transaction.commit()?;

        Ok(())
    }
}