use mesh_portal::version::latest::command::common::StateSrc;
use mesh_portal::version::latest::id::Point;

use crate::error::Error;
use crate::particle::{ParticleAssign, KindBase};
use crate::star::core::resource::driver::ParticleCoreDriver;
use crate::star::core::resource::state::StateStore;
use crate::star::StarSkel;

#[derive(Debug)]
pub struct StatelessCoreDriver {
    skel: StarSkel,
    resource_type: KindBase
}

impl StatelessCoreDriver {
    pub async fn new(skel: StarSkel, resource_type: KindBase) -> Self {
        StatelessCoreDriver {
            skel: skel.clone(),
            resource_type
        }
    }
}

#[async_trait]
impl ParticleCoreDriver for StatelessCoreDriver {

    fn resource_type(&self) -> KindBase {
        self.resource_type.clone()
    }


    async fn assign(
        &mut self,
        assign: ParticleAssign,
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
