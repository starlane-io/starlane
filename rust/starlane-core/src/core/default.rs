use std::collections::HashSet;
use std::convert::{TryFrom, TryInto};
use std::iter::FromIterator;
use std::str::FromStr;
use std::sync::Arc;

use rusqlite::types::ValueRef;
use rusqlite::{params, Connection, Transaction};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot};

use crate::app::ConfigSrc;
use crate::core::Host;
use crate::error::Error;
use crate::file_access::FileAccess;
use crate::frame::ResourceHostAction;
use crate::keys::ResourceKey;
use crate::message::Fail;
use crate::names::{Name, Specific};
use crate::resource;
use crate::resource::store::{
    ResourceStore, ResourceStoreAction, ResourceStoreCommand, ResourceStoreResult,
    ResourceStoreSqlLite,
};
use crate::resource::user::UserState;
use crate::resource::{
    AssignResourceStateSrc, DataTransfer, FileDataTransfer, LocalDataSrc, MemoryDataTransfer,
    Names, RemoteDataSrc, Resource, ResourceAddress, ResourceArchetype, ResourceAssign,
    ResourceIdentifier, ResourceKind, ResourceStatePersistenceManager, ResourceStateSrc,
    ResourceType,
};

#[derive(Debug)]
pub struct DefaultHost {
    store: ResourceStore,
}

impl DefaultHost {
    pub async fn new() -> Self {
        DefaultHost {
            store: ResourceStore::new().await,
        }
    }
}

#[async_trait]
impl Host for DefaultHost {
    #[instrument]
    async fn assign(
        &mut self,
        assign: ResourceAssign<AssignResourceStateSrc>,
    ) -> Result<Resource, Fail> {
        // if there is Initialization to do for assignment THIS is where we do it
        let data_transfer = match assign.state_src {
            AssignResourceStateSrc::Direct(data) => {
                let data_transfer: Arc<dyn DataTransfer> = Arc::new(MemoryDataTransfer::new(data));
                data_transfer
            }
            AssignResourceStateSrc::Hosted => Arc::new(MemoryDataTransfer::none()),
            AssignResourceStateSrc::None => Arc::new(MemoryDataTransfer::none()),
            AssignResourceStateSrc::InitArgs(_) => Arc::new(MemoryDataTransfer::none()),
        };

        let assign = ResourceAssign {
            stub: assign.stub,
            state_src: data_transfer,
        };

        Ok(self.store.put(assign).await?)
    }

    async fn get(&self, identifier: ResourceIdentifier) -> Result<Option<Resource>, Fail> {
        self.store.get(identifier).await
    }

    async fn state(&self, identifier: ResourceIdentifier) -> Result<RemoteDataSrc, Fail> {
        if let Option::Some(resource) = self.store.get(identifier.clone()).await? {
            Ok(RemoteDataSrc::Memory(resource.state_src().get().await?))
        } else {
            Err(Fail::ResourceNotFound(identifier))
        }
    }

    async fn delete(&self, identifier: ResourceIdentifier) -> Result<(), Fail> {
        unimplemented!()
    }
}
