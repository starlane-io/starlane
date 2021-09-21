use std::convert::TryInto;
use std::sync::Arc;

use clap::{App, AppSettings};
use yaml_rust::Yaml;

use starlane_resources::{AssignKind, AssignResourceStateSrc, Resource, ResourceAssign};
use starlane_resources::data::{BinSrc, DataSet, Meta};
use starlane_resources::message::Fail;

use crate::artifact::ArtifactRef;
use crate::error::Error;
use crate::resource::{ArtifactKind, ResourceAddress, ResourceKey, ResourceType};
use crate::resource::create_args::{artifact_bundle_address, create_args_artifact_bundle, space_address};
use crate::star::core::resource::host::Host;
use crate::star::core::resource::state::StateStore;
use crate::star::StarSkel;
use crate::watch::{Notification, Change, Topic, WatchSelector, Property};

#[derive(Debug)]
pub struct FileHost {
    skel: StarSkel,
    store: StateStore,
}

impl FileHost {
    pub async fn new(skel: StarSkel) -> Self {
        FileHost {
            skel: skel.clone(),
            store: StateStore::new(skel).await,
        }
    }
}

#[async_trait]
impl Host for FileHost {
    async fn assign(
        &self,
        assign: ResourceAssign<AssignResourceStateSrc<DataSet<BinSrc>>>,
    ) -> Result<DataSet<BinSrc>, Error> {
        let state = match assign.state_src {
            AssignResourceStateSrc::Direct(data) => data,
            AssignResourceStateSrc::Stateless => return Err("File cannot be stateless".into()),
            _ => {
                return Err("File must specify Direct state".into() )
            }
        };

        let state= self.store.put(assign.stub.key.clone(), state ).await?;

        let selector = WatchSelector{
            topic: Topic::Resource(assign.stub.key),
            property: Property::State
        };

        self.skel.watch_api.fire( Notification::new(selector, Change::State(state.clone()) ));

        Ok(state)
    }

    async fn has(&self, key: ResourceKey) -> bool {
        match self.store.has(key).await {
            Ok(v) => v,
            Err(_) => false,
        }
    }

    async fn get_state(&self, key: ResourceKey) -> Result<Option<DataSet<BinSrc>>, Error> {
        self.store.get(key).await
    }


    async fn update_state(&self,key: ResourceKey, state: DataSet<BinSrc> ) -> Result<(),Error> {

        self.store.put( key.clone(), state.clone() ).await?;

        let selector = WatchSelector{
            topic: Topic::Resource(key),
            property: Property::State
        };

        self.skel.watch_api.fire( Notification::new(selector, Change::State(state.clone()) ));


        Ok(())
    }


    async fn delete(&self, _identifier: ResourceKey) -> Result<(), Error> {
        unimplemented!()
    }
}

