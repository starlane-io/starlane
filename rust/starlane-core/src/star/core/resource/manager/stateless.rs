use mesh_portal_serde::version::v0_0_1::generic::resource::command::common::StateSrc;

use crate::error::Error;
use crate::mesh::serde::id::Address;
use crate::resource::{ResourceAssign, ResourceType};
use crate::star::core::resource::manager::ResourceManager;
use crate::star::core::resource::state::StateStore;
use crate::star::StarSkel;

#[derive(Debug)]
pub struct StatelessManager {
    skel: StarSkel,
    store: StateStore,
    resource_type: ResourceType
}

impl StatelessManager {
    pub async fn new(skel: StarSkel, resource_type: ResourceType ) -> Self {
        StatelessManager {
            skel: skel.clone(),
            store: StateStore::new(skel),
            resource_type
        }
    }
}

#[async_trait]
impl ResourceManager for StatelessManager {

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
