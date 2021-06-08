use std::collections::HashSet;
use std::convert::{TryFrom, TryInto};
use std::iter::FromIterator;
use std::str::FromStr;
use std::sync::Arc;
use std::path::PathBuf;

use rusqlite::Connection;
use tokio::sync::mpsc;

use crate::core::Host;
use crate::error::Error;
use crate::file::{FileAccess, FileEvent};
use crate::keys::ResourceKey;
use crate::message::Fail;
use crate::resource::{AssignResourceStateSrc, DataTransfer, MemoryDataTransfer, Path, Resource, ResourceAddress, ResourceAssign, ResourceStateSrc, ResourceType, ResourceCreationChamber};
use crate::resource::store::{ResourceStore, ResourceStoreAction, ResourceStoreCommand, ResourceStoreResult};
use crate::star::StarSkel;

use std::fs;

pub struct FileStoreHost {
    skel: StarSkel,
    file_access: FileAccess,
    store: ResourceStore,
}

impl FileStoreHost {
    pub async fn new(skel: StarSkel, file_access: FileAccess)->Result<Self,Error>{

        let mut file_access = file_access.with_path( "filesystems".to_string() ).await?;
        let rtn = FileStoreHost {
            skel: skel,
            file_access: file_access,
            store: ResourceStore::new().await,
        };

        rtn.watch().await?;

        Ok(rtn)
    }

    async fn watch(&self) -> Result<(),Error>{
        let mut event_rx = self.file_access.watch().await?;
        let store = self.store.clone();
        let dir = PathBuf::from(self.file_access.path());
        let root_path = fs::canonicalize(&dir)?.to_str().ok_or("turning path to string")?.to_string();
        tokio::spawn( async move {
            while let Option::Some(event) = event_rx.recv().await {
                Self::handle_event(root_path.clone(), event).await.unwrap();
            }
        } );
        Ok(())
    }

    async fn handle_event(  root_path: String, event: FileEvent ) -> Result<(),Error>{

        println!("event path {}", event.path );
        println!("root  path {}", root_path );
        let mut path = event.path.clone();
        for _ in 0..root_path.len() {
            path.remove(0);
        }
        println!("chomped path {}", path );
        println!("REceived Event: {:?}",event );
        Ok(())
        // first get filesystem
    }
}



#[async_trait]
impl Host for FileStoreHost {

    async fn assign(&mut self, assign: ResourceAssign<AssignResourceStateSrc>) -> Result<Resource, Fail> {
println!("$$$$ FILE RESOURCE ASSIGN: {}", assign.stub.archetype.kind );
        // if there is Initialization to do for assignment THIS is where we do it
       let data_transfer= match assign.state_src{
            AssignResourceStateSrc::Direct(data) => {
                let data_transfer:Arc<dyn DataTransfer> = Arc::new(MemoryDataTransfer::new(data));
                data_transfer
            },
            AssignResourceStateSrc::Hosted => {
                Arc::new(MemoryDataTransfer::none())
            }
        };

        match assign.stub.key.resource_type(){
            ResourceType::FileSystem => {
                // here we just ensure that a directory exists for the filesystem
                let path = Path::new(format!("/{}",assign.stub.key.to_string().as_str()).as_str() )?;
                self.file_access.mkdir(&path).await?;
            }
            ResourceType::File => {}
            rt => {
                return Err(Fail::WrongResourceType { expected: HashSet::from_iter(vec![ResourceType::FileSystem,ResourceType::File]), received: rt });
            }
        }

        let assign = ResourceAssign{
            stub: assign.stub,
            state_src: data_transfer
        };

        Ok(self.store.put( assign ).await?)
    }

    async fn get(&self, key: ResourceKey) -> Result<Option<Resource>, Fail> {
        self.store.get(key).await
    }

}