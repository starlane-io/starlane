use std::collections::HashSet;
use std::convert::{TryFrom, TryInto};
use std::iter::FromIterator;
use std::str::FromStr;
use std::sync::Arc;

use rusqlite::Connection;
use tokio::sync::mpsc;

use crate::core::Host;
use crate::error::Error;
use crate::file::FileAccess;
use crate::keys::ResourceKey;
use crate::message::Fail;
use crate::resource::{AssignResourceStateSrc, DataTransfer, MemoryDataTransfer, Path, Resource, ResourceAddress, ResourceAssign, ResourceStateSrc, ResourceType};
use crate::resource::store::{ResourceStore, ResourceStoreAction, ResourceStoreCommand, ResourceStoreResult};
use crate::star::StarSkel;

pub struct FileStoreHost {
    skel: StarSkel,
    file_access: Box<dyn FileAccess>,
    store: ResourceStore
}

impl FileStoreHost {
    pub async fn new(skel: StarSkel, file_access: Box<dyn FileAccess>)->Result<Self,Error>{
        let mut file_access = file_access.with_path( "filesystems".to_string() )?;
        Ok(FileStoreHost {
            skel: skel,
            file_access: file_access,
            store: ResourceStore::new().await
        })
    }
}

#[async_trait]
impl Host for FileStoreHost {

    async fn assign(&mut self, assign: ResourceAssign<AssignResourceStateSrc>) -> Result<Resource, Fail> {
        // if there is Initialization to do for assignment THIS is where we do it
        let data = match assign.state_src{
            AssignResourceStateSrc::Direct(data) => data
        };

        let data_transfer:Arc<dyn DataTransfer> = Arc::new(MemoryDataTransfer::new(data));

        match assign.key.resource_type(){
            ResourceType::FileSystem => {
                // here we just ensure that a directory exists for the filesystem
                let path = Path::new(assign.address.last_to_string()?.as_str() )?;
                self.file_access.mkdir(&path)?;
            }
            ResourceType::File => {}
            rt => {
                return Err(Fail::WrongResourceType { expected: HashSet::from_iter(vec![ResourceType::FileSystem,ResourceType::File]), received: rt });
            }
        }

        let assign = ResourceAssign{
            key: assign.key,
            address: assign.address,
            archetype: assign.archetype,
            state_src: data_transfer
        };

        Ok(self.store.put( assign ).await?)
    }

    async fn get(&self, key: ResourceKey) -> Result<Option<Resource>, Fail> {
        self.store.get(key).await
    }

}