use starlane_resources::{AssignResourceStateSrc, Resource, ResourceAssign};
use starlane_resources::data::{BinSrc, DataSet};
use starlane_resources::message::Fail;

use crate::error::Error;
use crate::resource::ResourceKey;
use crate::star::core::resource::host::Host;
use crate::star::core::resource::state::StateStore;
use crate::star::StarSkel;
use std::collections::hash_map::RandomState;
use std::collections::HashMap;

#[derive(Debug)]
pub struct StatelessHost {
    skel: StarSkel,
    store: StateStore,
}

impl StatelessHost {
    pub async fn new(skel: StarSkel) -> Self {
        StatelessHost {
            skel: skel.clone(),
            store: StateStore::new(skel).await,
        }
    }
}

#[async_trait]
impl Host for StatelessHost {

    async fn assign(
        &self,
        assign: ResourceAssign<AssignResourceStateSrc<DataSet<BinSrc>>>,
    ) -> Result<DataSet<BinSrc>, Error> {
        match assign.state_src {
            AssignResourceStateSrc::Stateless => {}
            AssignResourceStateSrc::CreateArgs(_) => {}
            _ => {
                return Err("must be stateless or empty create args".into());
            }
        };

        Ok(DataSet::new())
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

    async fn delete(&self, _identifier: ResourceKey) -> Result<(), Error> {
        unimplemented!()
    }

}
