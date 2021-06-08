use std::collections::HashSet;
use std::convert::{TryFrom, TryInto};
use std::iter::FromIterator;
use std::str::FromStr;
use std::sync::Arc;

use rusqlite::Connection;
use tokio::sync::mpsc;

use crate::core::Host;
use crate::error::Error;
use crate::file::{FileAccess, FileEvent};
use crate::keys::ResourceKey;
use crate::message::Fail;
use crate::resource::{AssignResourceStateSrc, DataTransfer, MemoryDataTransfer, Path, Resource, ResourceAddress, ResourceAssign, ResourceStateSrc, ResourceType};
use crate::resource::store::{ResourceStore, ResourceStoreAction, ResourceStoreCommand, ResourceStoreResult};
use crate::star::StarSkel;

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
        tokio::spawn( async move {
            while let Option::Some(event) = event_rx.recv().await {
                println!("REceived Event: {:?}",event );
            }
        } );
        Ok(())
    }
}



#[async_trait]
impl Host for FileStoreHost {

    async fn assign(&mut self, assign: ResourceAssign<AssignResourceStateSrc>) -> Result<Resource, Fail> {
println!("$$$$ FILE RESOURCE ASSIGN: {}", assign.stub.archetype.kind );
        // if there is Initialization to do for assignment THIS is where we do it
        let data = match assign.state_src{
            AssignResourceStateSrc::Direct(data) => data
        };

        let data_transfer:Arc<dyn DataTransfer> = Arc::new(MemoryDataTransfer::new(data));

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