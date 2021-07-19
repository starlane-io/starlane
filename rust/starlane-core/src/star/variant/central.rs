use std::convert::TryInto;
use std::str::FromStr;
use std::sync::Arc;
use std::thread;

use tokio::runtime::{Handle, Runtime};
use tokio::sync::oneshot;

use crate::error::Error;
use crate::message::resource;
use crate::resource::{
    create_args, ResourceAddress, ResourceArchetype, ResourceCreateStrategy, ResourceKind,
    ResourceLocation, ResourceRecord, ResourceRegistration, ResourceStub
};
use crate::resource::ResourceKey;
use crate::star::{PublicKeySource, StarKey, StarSkel};
use crate::star::variant::{StarVariant, StarVariantCommand};
use crate::starlane::api::{SpaceApi, StarlaneApi};

pub struct CentralVariant {
    skel: StarSkel,
}

impl CentralVariant {
    pub async fn new(data: StarSkel) -> CentralVariant {
        CentralVariant { skel: data.clone() }
    }
}

#[async_trait]
impl StarVariant for CentralVariant {
    fn init(&self, tx: oneshot::Sender<Result<(), Error>>) {
        let root_resource = ResourceRecord {
            stub: ResourceStub {
                key: ResourceKey::Root,
                address: ResourceAddress::from_str("<Root>").unwrap(),
                archetype: ResourceArchetype {
                    kind: ResourceKind::Root,
                    specific: None,
                    config: None,
                },
                owner: None,
            },
            location: ResourceLocation {
                host: StarKey::central(),
            },
        };

        let registration = ResourceRegistration {
            resource: root_resource,
            info: None,
        };


        let skel = self.skel.clone();

         tokio::spawn( async move {
            let registry = skel.registry.as_ref().unwrap();
            registry.register(registration).await.unwrap();
            let starlane_api = StarlaneApi::new(skel.star_tx.clone());
            let result =   Self::ensure(starlane_api).await;
            if let Result::Err(error) = result.as_ref() {
                error!("Central Init Error: {}",error.to_string() );
            }
            tx.send(result);
         });
    }
}

impl CentralVariant {
    async fn ensure(starlane_api: StarlaneApi) -> Result<(), Error> {
        let mut creation = starlane_api.create_space("hyperspace", "Hyper Space")?;
        creation.set_strategy(ResourceCreateStrategy::Ensure);
        let space_api = creation.submit().await?;

        let mut creation = space_api.create_user("hyperuser@starlane.io")?;
        creation.set_strategy(ResourceCreateStrategy::Ensure);
        creation.submit().await?;

        let mut creation = space_api.create_sub_space("starlane")?;
        creation.set_strategy(ResourceCreateStrategy::Ensure);
        creation.submit().await?;

        let mut creation = space_api.create_domain("localhost")?;
        creation.set_strategy(ResourceCreateStrategy::Ensure);
        creation.submit().await?;

        let init_args = Arc::new(create_args::create_init_args_artifact_bundle()?);
        let creation = starlane_api.create_artifact_bundle(&create_args::artifact_bundle_address(), init_args ).await?;
        creation.submit().await?;

        Ok(())
    }
}
