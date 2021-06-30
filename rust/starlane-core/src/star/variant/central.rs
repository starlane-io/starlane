use crate::error::Error;
use crate::keys::ResourceKey;
use crate::resource::{
    ResourceAddress, ResourceArchetype, ResourceCreateStrategy, ResourceKind, ResourceLocation,
    ResourceRecord, ResourceRegistration, ResourceStub,
};
use crate::star::variant::{StarVariant, StarVariantCommand};
use crate::star::{PublicKeySource, StarKey, StarSkel};
use crate::starlane::api::{SpaceApi, StarlaneApi};
use std::convert::TryInto;
use std::str::FromStr;
use tokio::sync::oneshot;

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
                gathering: None,
            },
        };

        let registration = ResourceRegistration {
            resource: root_resource,
            info: None,
        };


        let skel = self.skel.clone();
        tokio::spawn(async move {
           let registry = skel.registry.as_ref().unwrap();
           registry.register(registration).await.unwrap();
           let starlane_api = StarlaneApi::new(skel.star_tx.clone());
           tx.send(Self::ensure(starlane_api).await);
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

        let mut creation = space_api.create_sub_space("default")?;
        creation.set_strategy(ResourceCreateStrategy::Ensure);
        creation.submit().await?;

        let mut creation = space_api.create_domain("localhost")?;
        creation.set_strategy(ResourceCreateStrategy::Ensure);
        creation.submit().await?;

        Ok(())
    }
}
