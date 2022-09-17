use cosmic_universe::id::BaseKind;
use cosmic_universe::hyper::Assign;
use mesh_portal::version::latest::command::common::StateSrc;
use mesh_portal::version::latest::id::Point;

use crate::error::Error;
use crate::star::core::particle::driver::ParticleCoreDriver;
use crate::star::core::particle::state::StateStore;
use crate::star::StarSkel;

#[derive(Debug)]
pub struct StatelessCoreDriver {
    skel: StarSkel,
    resource_type: BaseKind,
}

impl StatelessCoreDriver {
    pub async fn new(skel: StarSkel, resource_type: BaseKind) -> Self {
        StatelessCoreDriver {
            skel: skel.clone(),
            resource_type,
        }
    }
}

#[async_trait]
impl ParticleCoreDriver for StatelessCoreDriver {
    fn kind(&self) -> BaseKind {
        self.resource_type.clone()
    }

    async fn assign(&mut self, assign: Assign) -> Result<(), Error> {
        match assign.state {
            StateSrc::None => {}
            StateSrc::Substance(_) => {
                return Err("must be stateless".into());
            }
        };
        Ok(())
    }
}
