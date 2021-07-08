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
use crate::star::StarSkel;
use crate::artifact::{ArtifactRef, ArtifactKind};
use clap::App;

#[derive(Debug)]
pub struct DefaultHost {
    skel: StarSkel,
    store: ResourceStore,
}

impl DefaultHost {
    pub async fn new(skel: StarSkel) -> Self {
        DefaultHost {
            skel: skel,
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
info!("DefaultHost assign...");
        // if there is Initialization to do for assignment THIS is where we do it
        let data_transfer = match assign.state_src {
            AssignResourceStateSrc::Direct(data) => {
                let data_transfer: Arc<dyn DataTransfer> = Arc::new(MemoryDataTransfer::new(data));
                data_transfer
            }
            AssignResourceStateSrc::Hosted => Arc::new(MemoryDataTransfer::none()),
            AssignResourceStateSrc::None => Arc::new(MemoryDataTransfer::none()),
            AssignResourceStateSrc::InitArgs(ref args) =>  {
                Arc::new(if args.trim().is_empty() && assign.stub.archetype.kind.init_clap_config()?.is_none() {
                    MemoryDataTransfer::none()
                } else if assign.stub.archetype.kind.init_clap_config()?.is_none(){
                    return Err(format!("resource {} does not take init args",assign.archetype().kind.to_string()).into());
                }
                else {
info!("enter");
                    let artifact = assign.archetype().kind.init_clap_config()?.expect("expected init clap config");
info!("got init clapConfig");
                    let mut cache = self.skel.caches.create();
println!("artifact is::: {}",artifact.to_string());
info!("got self.skel.caches.create()");
                    let artifact_ref = ArtifactRef::new(artifact.clone(), ArtifactKind::Raw );
                    match cache.cache(vec![artifact_ref]).await {
                        Ok(_) => {}
                        Err(err) => {error!("{}",err);
                            return Err(err.into());
                        }
                    };
info!("artifact cached: {}", artifact.to_string() );
                    let caches = cache.to_caches().await?;
info!("cache.to_caches()." );
                    let yaml_config = caches.raw.get(&artifact).ok_or("expected artifact")?;
info!("caches.raw.get()." );
                    let data = (*yaml_config.data()).clone();
                    let data = String::from_utf8(data)?;
info!("String::from_utf8()." );
                    let yaml = clap::YamlLoader::load_from_str(data.as_str() )?;
info!("clap::YamlLoader::load_from_str(data.as_str() )" );
                    let yaml = yaml.get(0).ok_or("expected at least one Yaml expression")?.clone();
info!("at least one yaml" );
                    let mut app = App::from(&yaml);
info!("App::from(&yaml)" );
                    let matches = app.get_matches_from_safe(args.split(" "))?;
                    // now not sure what to do with matches
println!("seems to have worked....");
                    MemoryDataTransfer::none()
                })
            }


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
