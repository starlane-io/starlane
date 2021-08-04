use crate::star::StarSkel;
use crate::star::core::resource::state::StateStore;
use crate::star::core::resource::host::Host;
use crate::resource::{ResourceAssign, AssignResourceStateSrc, ResourceKey, Resource};
use crate::data::{DataSet, BinSrc};
use crate::message::Fail;
use crate::error::Error;

#[derive(Debug)]
pub struct DomainHost {
    skel: StarSkel,
    store: StateStore,
}

impl DomainHost {
    pub async fn new(skel: StarSkel) -> Self {
        DomainHost {
            skel: skel.clone(),
            store: StateStore::new(skel).await,
        }
    }
}

#[async_trait]
impl Host for DomainHost {
    async fn assign(
        &self,
        assign: ResourceAssign<AssignResourceStateSrc<DataSet<BinSrc>>>,
    ) -> Result<DataSet<BinSrc>, Fail> {
        match assign.state_src {
            AssignResourceStateSrc::Stateless => {
            },
            _ =>  {
                return Err("domain must be stateless".into());
            }
        };


        Ok(DataSet::new())
    }

    async fn has(&self, key: ResourceKey) -> bool {
        match self.store.has(key).await
        {
            Ok(v) => v,
            Err(_) => false
        }
    }

    async fn get(&self, key: ResourceKey) -> Result<Option<DataSet<BinSrc>>, Fail> {
        self.store.get(key).await
    }

    async fn delete(&self, _identifier: ResourceKey ) -> Result<(), Fail> {
        unimplemented!()
    }
}