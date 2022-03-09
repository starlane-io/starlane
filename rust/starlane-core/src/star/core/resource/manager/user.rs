use mesh_portal::version::latest::command::common::StateSrc;
use crate::error::Error;
use crate::resource::{ResourceAssign, ResourceType};
use crate::star::core::resource::manager::ResourceManager;
use crate::star::StarSkel;

#[derive(Debug)]
pub struct UserCoreManager {
    skel: StarSkel,
}

impl UserCoreManager {
    pub async fn new(skel: StarSkel) -> Self {
        UserCoreManager {
            skel: skel.clone(),
        }
    }
}

#[async_trait]
impl ResourceManager for UserCoreManager {

    fn resource_type(&self) -> ResourceType {
        ResourceType::User
    }


    async fn assign(
        &mut self,
        assign: ResourceAssign,
    ) -> Result<(), Error> {
        match assign.state {
            StateSrc::Stateless => {
            }
            StateSrc::StatefulDirect(_) => {
                return Err("User must be stateless".into());
            }
        };


        Ok(())
    }


}
