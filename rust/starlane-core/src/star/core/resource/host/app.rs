use crate::data::{BinSrc, DataSet};
use crate::error::Error;
use crate::message::Fail;
use crate::resource::{AssignResourceStateSrc, Resource, ResourceAssign, ResourceKey, ResourceAddress,ResourceType,ArtifactKind};
use crate::star::core::resource::host::Host;
use crate::star::core::resource::state::StateStore;
use crate::star::StarSkel;
use crate::resource::create_args::{create_args_artifact_bundle, artifact_bundle_address, space_address};
use crate::artifact::ArtifactRef;
use clap::{App, AppSettings};
use yaml_rust::Yaml;
use starlane_resources::data::Meta;
use std::convert::TryInto;
use std::sync::Arc;
use crate::app::ConfigSrc;
use crate::cache::ArtifactItem;
use crate::config::app::AppConfig;

#[derive(Debug)]
pub struct AppHost {
    skel: StarSkel,
    store: StateStore,
}

impl AppHost {
    pub async fn new(skel: StarSkel) -> Self {
        AppHost {
            skel: skel.clone(),
            store: StateStore::new(skel).await,
        }
    }
}

#[async_trait]
impl Host for AppHost {
    async fn assign(
        &self,
        assign: ResourceAssign<AssignResourceStateSrc<DataSet<BinSrc>>>,
    ) -> Result<DataSet<BinSrc>, Fail> {
        match assign.state_src {
            AssignResourceStateSrc::Direct(data) => return Err("App cannot be stateful".into()),
            AssignResourceStateSrc::Stateless => {
            }
            AssignResourceStateSrc::CreateArgs(args) => {
                return Err("App doesn't currently accept command line args.".into())
            }
        }

        let app_config_artifact = match assign.stub.archetype.config {
            None => return Err("App requires a config".into() ),
            Some(ConfigSrc::Artifact(artifact)) => {
println!("artifact : {}", artifact.to_string());
                artifact
            }
            _ => return Err("App requires a config referencing an artifact".into() ),
        };

        let factory = self.skel.machine.get_proto_artifact_caches_factory().await?;
        let mut proto = factory.create();
        let app_config_artifact_ref = ArtifactRef::new(app_config_artifact.clone(), ArtifactKind::AppConfig );
        proto.cache(vec![app_config_artifact_ref]).await?;
        let caches = proto.to_caches().await?;
        let app_config = caches.app_configs.get(&app_config_artifact).ok_or::<Error>(format!("expected app_config").into())?;


        println!("App config loaded!");

        println!("main: {}", app_config.main.address.to_string() );

        Ok(DataSet::new())
    }

    async fn has(&self, key: ResourceKey) -> bool {
        match self.store.has(key).await {
            Ok(v) => v,
            Err(_) => false,
        }
    }

    async fn get(&self, key: ResourceKey) -> Result<Option<DataSet<BinSrc>>, Fail> {
        self.store.get(key).await
    }

    async fn delete(&self, _identifier: ResourceKey) -> Result<(), Fail> {
        unimplemented!()
    }
}

impl AppHost {
    async fn create_from_args(&self, args: String) -> Result<DataSet<BinSrc>,Error> {
        unimplemented!();
    }
}