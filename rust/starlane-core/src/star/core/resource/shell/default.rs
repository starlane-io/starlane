
use crate::error::Error;
use crate::resource::{ResourceType, AssignResourceStateSrc, ResourceAssign, Kind};
use crate::star::core::resource::shell::Host;
use crate::star::core::resource::state::StateStore;
use crate::star::StarSkel;
use std::collections::hash_map::RandomState;
use std::collections::HashMap;
use mesh_portal_serde::version::v0_0_1::generic::resource::command::common::StateSrc;
use crate::mesh::serde::id::Address;

#[derive(Debug)]
pub struct StatelessHost {
    skel: StarSkel,
    store: StateStore,
    resource_type: ResourceType
}

impl StatelessHost {
    pub async fn new(skel: StarSkel, resource_type: ResourceType ) -> Self {
        StatelessHost {
            skel: skel.clone(),
            store: StateStore::new(skel).await,
            resource_type
        }
    }
}

#[async_trait]
impl Host for StatelessHost {

    fn resource_type(&self) -> ResourceType {
        self.resource_type.clone()
    }


    async fn assign(
        &self,
        assign: ResourceAssign,
    ) -> Result<(), Error> {
        match assign.state {
            StateSrc::Stateless => {
            }
            StateSrc::StatefulDirect(_) => {
                return Err("must be stateless".into());
            }
        };

        Ok(())
    }

    async fn has(&self, address: Address ) -> bool {
        match self.store.has(address).await {
            Ok(v) => v,
            Err(_) => false,
        }
    }

}
