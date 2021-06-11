use crate::star::{StarSkel};
use crate::star::variant::{StarVariant, StarVariantCommand};
use crate::error::Error;

pub struct WebVariant
{
    skel: StarSkel,
}

impl WebVariant
{
    pub async fn new(skel: StarSkel) -> WebVariant
    {
        WebVariant
        {
            skel: skel.clone(),
        }
    }
}


#[async_trait]
impl StarVariant for WebVariant
{
    async fn init(&self) -> Result<(), Error> {
        Ok(())
    }
}