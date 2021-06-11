use crate::error::Error;
use crate::resource::ResourceAddress;
use crate::star::{PublicKeySource, StarSkel};
use crate::star::variant::{StarVariant, StarVariantCommand};
use crate::starlane::api::StarlaneApi;
use tokio::sync::oneshot;

pub struct CentralVariant
{
    skel: StarSkel,
}

impl CentralVariant
{
    pub async fn new(data: StarSkel) -> CentralVariant
    {
        CentralVariant
        {
            skel: data.clone()
        }
    }
}


#[async_trait]
impl StarVariant for CentralVariant
{
    async fn init(&self, tx: oneshot::Sender<Result<(), Error>>)
    {
        let starlane_api = StarlaneApi::new(self.skel.star_tx.clone());
        tokio::spawn(async move {
            tx.send(Self::ensure(starlane_api).await );
        });
    }
}

impl CentralVariant {
    async fn ensure( starlane_api: StarlaneApi ) -> Result<(),Error>
    {
        let space_api = starlane_api.create_space("hyperspace", "Hyper Space").await?;
        space_api.create_user("hyperuser@starlane.io").await?;
        space_api.create_sub_space("default").await?;
        space_api.create_domain("localhost").await?;

        Ok(())
    }
}




