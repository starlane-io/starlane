use crate::star::{StarSkel, RegistryBacking, RegistryBackingSqlLite, StarVariant, StarVariantCommand};
use crate::star::pledge::StarHandleBacking;

pub struct SpaceVariant
{
    skel: StarSkel,
    registry: Box<dyn RegistryBacking>,
    star_handles: StarHandleBacking
}

impl SpaceVariant
{
    pub async fn new(data: StarSkel) ->Self
    {
        SpaceVariant {
            skel: data.clone(),
            registry: Box::new(RegistryBackingSqlLite::new().await ),
            star_handles: StarHandleBacking::new().await
        }
    }
}

#[async_trait]
impl StarVariant for SpaceVariant {

    async fn handle(&mut self, command: StarVariantCommand) {

    }

}