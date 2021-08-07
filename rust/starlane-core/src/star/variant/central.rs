use std::str::FromStr;
use std::sync::Arc;

use tokio::sync::oneshot;

use crate::error::Error;

use crate::resource::ResourceKey;
use crate::resource::{
    create_args, AddressCreationSrc, AssignResourceStateSrc, KeyCreationSrc, ResourceAddress,
    ResourceArchetype, ResourceCreate, ResourceCreateStrategy, ResourceKind, ResourceLocation,
    ResourceRecord, ResourceRegistration, ResourceStub,
};
use crate::star::variant::StarVariant;
use crate::star::{StarKey, StarSkel};
use crate::starlane::api::StarlaneApi;

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

        tokio::spawn(async move {
            let registry = skel.registry.as_ref().unwrap();
            registry.register(registration).await.unwrap();
            let starlane_api = StarlaneApi::new(skel.surface_api.clone());
            let result = Self::ensure(starlane_api).await;
            if let Result::Err(error) = result.as_ref() {
                error!("Central Init Error: {}", error.to_string());
            }
            tx.send(result);
        });
    }
}

impl CentralVariant {
    async fn ensure(starlane_api: StarlaneApi) -> Result<(), Error> {
        println!("ensuring hyperspace...");
        let mut creation = starlane_api.create_space("hyperspace", "Hyper Space")?;
        creation.set_strategy(ResourceCreateStrategy::Ensure);
        let space_api = creation.submit().await?;
        println!("hyperspace ensured.");

        println!("ensuring subspace...");
        let mut creation = space_api.create_sub_space("starlane", "Starlane")?;
        creation.set_strategy(ResourceCreateStrategy::Ensure);
        let subspace_api = creation.submit().await?;
        println!("subspace ensured.");

        /*
                let mut creation = space_api.create_user("hyperuser@starlane.io")?;
                creation.set_strategy(ResourceCreateStrategy::Ensure);
                creation.submit().await?;
        println!("hyperuser ensured.");

                 */

        let mut creation = space_api.create_domain("localhost")?;
        creation.set_strategy(ResourceCreateStrategy::Ensure);
        creation.submit().await?;
        println!("localhost ensured.");

        {
            println!(
                "ensuring artifact: {}",
                create_args::artifact_bundle_address().to_string()
            );
            let address: ResourceAddress = create_args::artifact_bundle_address().into();
            let mut creation = subspace_api
                .create_artifact_bundle_versions(address.parent().unwrap().name().as_str())?;
            creation.set_strategy(ResourceCreateStrategy::Ensure);
            let artifact_bundle_versions_api = creation.submit().await?;

            let version = semver::Version::from_str(address.name().as_str())?;
            let mut creation = artifact_bundle_versions_api.create_artifact_bundle(
                version,
                Arc::new(create_args::create_args_artifact_bundle()?),
            )?;
            creation.set_strategy(ResourceCreateStrategy::Ensure);
            creation.submit().await?;
        }
        println!("created artifact bundle.");

        Ok(())
    }
}
