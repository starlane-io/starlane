use crate::data::{BinSrc, DataSet};
use crate::error::Error;
use crate::message::Fail;
use crate::resource::{AssignResourceStateSrc, Resource, ResourceAssign, ResourceKey};
use crate::star::core::resource::host::Host;
use crate::star::core::resource::state::StateStore;
use crate::star::StarSkel;

#[derive(Debug)]
pub struct SpaceHost {
    skel: StarSkel,
    store: StateStore,
}

impl SpaceHost {
    pub async fn new(skel: StarSkel) -> Self {
        SpaceHost {
            skel: skel.clone(),
            store: StateStore::new(skel).await,
        }
    }
}

#[async_trait]
impl Host for SpaceHost {
    async fn assign(
        &self,
        assign: ResourceAssign<AssignResourceStateSrc<DataSet<BinSrc>>>,
    ) -> Result<DataSet<BinSrc>, Fail> {
        let state = match assign.state_src {
            AssignResourceStateSrc::Direct(data) => data,
            AssignResourceStateSrc::Stateless => return Err("space cannot be stateless".into()),
            AssignResourceStateSrc::CreateArgs(ref args) => {
                if args.trim().is_empty() && assign.stub.archetype.kind.init_clap_config().is_none()
                {
                    DataSet::new()
                } else if assign.stub.archetype.kind.init_clap_config().is_none() {
                    return Err(format!(
                        "resource {} does not take init args",
                        assign.archetype().kind.to_string()
                    )
                    .into());
                } else {
                    unimplemented!()
                }
            }
        };

        let assign = ResourceAssign {
            stub: assign.stub,
            state_src: state,
        };

        Ok(self.store.put(assign).await?)
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
