use crate::error::Error;
use crate::resource::ResourceAddress;
use crate::star::{PublicKeySource, StarSkel};
use crate::star::variant::{StarVariant, StarVariantCommand};
use crate::starlane::api::StarlaneApi;

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
    async fn init(&self) -> Result<(),Error>
    {
       self.ensure().await
    }
}

impl CentralVariant {
    async fn ensure(&self) -> Result<(),Error>{
        self.ensure_hyperspace().await?;
        self.ensure_user(&ResourceAddress::for_space("hyperspace").unwrap(),"hyperuser@starlane.io").await?;
        self.ensure_subspace(&ResourceAddress::for_space("hyperspace").unwrap(),"default").await?;
        self.ensure_localhost_domain().await?;
        Ok(())
    }

    async fn ensure_hyperspace(&self)->Result<(),Error>{
        let starlane_api = StarlaneApi::new(self.skel.star_tx.clone());
        starlane_api.create_space("hyperspace", "HyperSpace").await?;

        Ok(())
    }

    async fn ensure_user(&self, space_address: &ResourceAddress, email: &str ) ->Result<(),Error>{
        let starlane_api = StarlaneApi::new(self.skel.star_tx.clone());
        let space_api = starlane_api.get_space(space_address.clone().into() ).await?;
        space_api.create_user(email).await?;
        Ok(())
    }

    async fn ensure_subspace(&self, space_address: &ResourceAddress, sub_space: &str ) ->Result<(),Error>{
        let starlane_api = StarlaneApi::new(self.skel.star_tx.clone());
        let space_api = starlane_api.get_space(space_address.clone().into()).await?;
        space_api.create_sub_space(sub_space).await?;
        Ok(())
    }

    async fn ensure_localhost_domain(&self) ->Result<(),Error>{
        let space_address = ResourceAddress::for_space("hyperspace")?;
        let starlane_api = StarlaneApi::new(self.skel.star_tx.clone());
        let space_api = starlane_api.get_space(space_address.clone().into()).await?;
        space_api.create_domain("localhost").await?;
        Ok(())
    }
}

