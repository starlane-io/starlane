use std::str::FromStr;
use std::sync::Arc;

use tokio::sync::{mpsc, oneshot};

use starlane_resources::{AddressCreationSrc, AssignResourceStateSrc, KeyCreationSrc, ResourceArchetype, ResourceCreate, ResourceCreateStrategy, ResourceStub, ResourcePath, ConfigSrc};

use crate::error::Error;
use crate::resource::{create_args, ResourceAddress, ResourceKind, ResourceRecord, ResourceRegistration, ResourceLocation};
use crate::resource::ResourceKey;
use crate::star::{StarKey, StarSkel};
use crate::star::variant::{FrameVerdict, VariantCall};
use crate::starlane::api::StarlaneApi;
use crate::util::{AsyncProcessor, AsyncRunner};

pub struct CentralVariant {
    skel: StarSkel,
}

impl CentralVariant {
    pub fn start(skel: StarSkel, rx: mpsc::Receiver<VariantCall>) {
        AsyncRunner::new(
            Box::new(Self { skel: skel.clone() }),
            skel.variant_api.tx.clone(),
            rx,
        );
    }
}

#[async_trait]
impl AsyncProcessor<VariantCall> for CentralVariant {
    async fn process(&mut self, call: VariantCall) {
        match call {
            VariantCall::Init(tx) => {
                self.init(tx);
            }
            VariantCall::Frame { frame, session:_, tx } => {
                tx.send(FrameVerdict::Handle(frame));
            }
        }
    }
}


impl CentralVariant {
    fn init(&self, tx: oneshot::Sender<Result<(), Error>>) {
        let root_resource = ResourceRecord {
            stub: ResourceStub {
                key: ResourceKey::Root,
                address: ResourcePath::root(),
                archetype: ResourceArchetype {
                    kind: ResourceKind::Root,
                    specific: None,
                    config: ConfigSrc::None,
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

        let mut creation = starlane_api.create_space("space", "Space")?;
        creation.set_strategy(ResourceCreateStrategy::Ensure);
        let space_api = creation.submit().await?;

        {

            let address = create_args::artifact_bundle_address();
            let mut creation = space_api
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

        Ok(())
    }
}
