use std::convert::{TryFrom, TryInto};
use std::str::FromStr;
use std::sync::Arc;

use rusqlite::Connection;
use tokio::sync::mpsc;

use crate::core::Host;
use crate::error::Error;
use crate::keys::ResourceKey;
use crate::message::Fail;
use crate::resource::{AssignResourceStateSrc, Resource, ResourceAddress, ResourceAssign, ResourceStateSrc, FileAccess, DataTransfer, MemoryDataTransfer, ResourceType, Path};
use crate::resource::store::{ResourceStoreAction, ResourceStoreCommand, ResourceStoreResult, ResourceStore};
use std::collections::HashSet;
use std::iter::FromIterator;


pub struct FileStoreHost {
    file_access: Box<dyn FileAccess>,
    store: ResourceStore
}

impl FileStoreHost {
    pub async fn new(file_access: Box<dyn FileAccess>)->Self{
        FileStoreHost {
            file_access: file_access,
            store: ResourceStore::new().await
        }
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