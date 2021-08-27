
use crate::data::{BinSrc, DataSet};
use crate::error::Error;
use crate::message::Fail;
use crate::resource::{AssignResourceStateSrc, Resource, ResourceAssign, ResourceKey};
use crate::star::core::resource::host::Host;
use crate::star::core::resource::state::StateStore;
use crate::star::StarSkel;

#[derive(Debug)]
pub struct MechtronHost {
    skel: StarSkel,
    store: StateStore,
}

impl MechtronHost {
    pub async fn new(skel: StarSkel) -> Self {
        MechtronHost {
            skel: skel.clone(),
            store: StateStore::new(skel).await,
        }
    }
}

#[async_trait]
impl Host for MechtronHost {
    async fn assign(
        &self,
        assign: ResourceAssign<AssignResourceStateSrc<DataSet<BinSrc>>>,
    ) -> Result<DataSet<BinSrc>, Fail> {
        match assign.state_src {
            AssignResourceStateSrc::Stateless => {}
            _ => {
                return Err("currently only supporting stateless mechtrons".into());
            }
        };

println!("ASSIGN MECHTRON!");

        Ok(DataSet::new())
    }

    async fn has(&self, key: ResourceKey) -> bool {
        match self.store.has(key).await {
            Ok(v) => v,
            Err(_) => false,
        }
    }

    async fn get(&self, key: ResourceKey) -> Result<Option<DataSet<BinSrc>>, Fail> {
        self.store.get(key).await
    }

    async fn delete(&self, _identifier: ResourceKey) -> Result<(), Fail> {
        unimplemented!()
    }
}
