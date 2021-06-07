use std::collections::HashSet;
use std::convert::{TryFrom, TryInto};
use std::iter::FromIterator;
use std::str::FromStr;
use std::sync::Arc;

use rusqlite::{Connection, params, Transaction};
use rusqlite::types::ValueRef;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot};

use crate::app::ConfigSrc;
use crate::core::Host;
use crate::error::Error;
use crate::file::FileAccess;
use crate::frame::ResourceHostAction;
use crate::keys::{ResourceKey, SpaceId};
use crate::message::Fail;
use crate::names::{Name, Specific};
use crate::resource::{AssignResourceStateSrc, DataTransfer, FileDataTransfer, LocalDataSrc, MemoryDataTransfer, Names, Resource, ResourceAddress, ResourceArchetype, ResourceAssign, ResourceKind, ResourceStatePersistenceManager, ResourceStateSrc, ResourceType};
use crate::resource;
use crate::resource::space::{Space, SpaceState};
use crate::resource::store::{ResourceStore, ResourceStoreAction, ResourceStoreCommand, ResourceStoreResult, ResourceStoreSqlLite};
use crate::resource::user::UserState;

pub struct SpaceHost {
  store: ResourceStore
}

impl SpaceHost {
    pub async fn new()->Self{
        SpaceHost {
            store: ResourceStore::new().await
        }
    }
}

#[async_trait]
impl Host for SpaceHost {

    async fn assign(&mut self, assign: ResourceAssign<AssignResourceStateSrc>) -> Result<Resource, Fail> {
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

        Ok(self.store.put( assign ).await?)
    }

    async fn get(&self, key: ResourceKey) -> Result<Option<Resource>, Fail> {
        self.store.get(key).await
    }

}