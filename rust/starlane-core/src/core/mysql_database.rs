use std::collections::HashSet;
use std::convert::{TryFrom, TryInto};
use std::iter::FromIterator;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use rusqlite::Connection;
use tokio::sync::{mpsc, Mutex};

use crate::core::Host;
use crate::error::Error;
use crate::file_access::{FileAccess, FileEvent};
use crate::keys::{FileSystemKey, ResourceKey};
use crate::message::Fail;
use crate::resource::store::{
    ResourceStore, ResourceStoreAction, ResourceStoreCommand, ResourceStoreResult,
};
use crate::resource::{
    AddressCreationSrc, ArtifactBundleKind, AssignResourceStateSrc, DataTransfer, FileKind,
    KeyCreationSrc, MemoryDataTransfer, Path, RemoteDataSrc, Resource, ResourceAddress,
    ResourceArchetype, ResourceAssign, ResourceCreate, ResourceCreateStrategy,
    ResourceCreationChamber, ResourceIdentifier, ResourceKind, ResourceStateSrc, ResourceStub,
    ResourceType,
};
use crate::star::StarSkel;

use crate::artifact::ArtifactBundleKey;
use crate::util;
use std::fs;
use std::fs::File;
use std::io::Write;
use tempdir::TempDir;

pub struct MySQLDatabaseCore {
    skel: StarSkel,
    store: ResourceStore,
    host: String,
    password: String
}

impl MySQLDatabaseCore {
    pub async fn new(skel: StarSkel) -> Result<Self, Error> {

        if std::env::var("MYSQL_CLUSTER_HOST").is_err() || std::env::var("MYSQL_CLUSTER_PASSWORD").is_err() {
            eprintln!("FATAL: expected environment variables not set: 'MYSQL_CLUSTER_HOST' and 'MYSQL_CLUSTER_PASSWORD'");
        }

        /*
        let rtn = MySQLDatabaseCore {
            skel: skel,
            store: ResourceStore::new().await,
            url: std::env::var("MYSQL_CLUSTER_HOST")?,
            password: std::env::var("MYSQL_CLUSTER_PASSWORD")?
        };

         */

        let rtn = MySQLDatabaseCore {
            skel: skel,
            store: ResourceStore::new().await,
            host: "localhost".to_string(),
            password: "password".to_string()
        };


        Ok(rtn)
    }
}


#[async_trait]
impl Host for MySQLDatabaseCore {
    async fn assign(
        &mut self,
        assign: ResourceAssign<AssignResourceStateSrc>,
    ) -> Result<Resource, Fail> {

        let data_transfer: Arc<dyn DataTransfer> = Arc::new(MemoryDataTransfer::none());

        let assign = ResourceAssign {
            stub: assign.stub.clone(),
            state_src: data_transfer,
        };

        let resource = self.store.put(assign).await?;
        Ok(resource)
    }

    async fn get(&self, identifier: ResourceIdentifier) -> Result<Option<Resource>, Fail> {
        self.store.get(identifier).await
    }

    async fn state(&self, identifier: ResourceIdentifier) -> Result<RemoteDataSrc, Fail> {
        if let Ok(Option::Some(resource)) = self.store.get(identifier.clone()).await {
            Ok(RemoteDataSrc::None)
        } else {
            Err(Fail::ResourceNotFound(identifier))
        }
    }

    async fn delete(&self, identifier: ResourceIdentifier) -> Result<(), Fail> {
        unimplemented!("I don't know how to DELETE yet.");
        Ok(())
    }
}