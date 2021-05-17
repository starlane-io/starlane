use crate::star::{StarSkel, ResourceRegistryBacking, ResourceRegistryBackingSqLite, StarVariant, StarVariantCommand};
use crate::star::pledge::StarHandleBacking;

pub struct SpaceVariant
{
    skel: StarSkel,
    star_handles: StarHandleBacking
}

impl SpaceVariant
{
    pub async fn new(data: StarSkel) ->Self
    {
        SpaceVariant {
            skel: data.clone(),
            star_handles: StarHandleBacking::new().await
        }
    }
}

#[async_trait]
impl StarVariant for SpaceVariant {

    async fn handle(&mut self, command: StarVariantCommand) {

    }

}