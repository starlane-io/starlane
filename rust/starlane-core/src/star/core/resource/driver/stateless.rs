use mesh_portal::version::latest::command::common::StateSrc;
use mesh_portal::version::latest::id::Address;

use crate::error::Error;
use crate::resource::{ResourceAssign, ResourceType};
use crate::star::core::resource::driver::ResourceCoreDriver;
use crate::star::core::resource::state::StateStore;
use crate::star::StarSkel;

#[derive(Debug)]
pub struct StatelessCoreDriver {
    skel: StarSkel,
    resource_type: ResourceType
}

impl StatelessCoreDriver {
    pub async fn new(skel: StarSkel, resource_type: ResourceType ) -> Self {
        StatelessCoreDriver {
            skel: skel.clone(),
            resource_type
        }
    }
}

#[async_trait]
impl ResourceCoreDriver for StatelessCoreDriver {

    fn resource_type(&self) -> ResourceType {
        self.resource_type.clone()
    }


    async fn assign(
        &mut self,
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


}
