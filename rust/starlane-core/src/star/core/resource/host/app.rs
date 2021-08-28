use std::convert::TryInto;
use std::sync::Arc;

use clap::{App, AppSettings};
use yaml_rust::Yaml;

use starlane_resources::{AssignResourceStateSrc, Resource, ResourceAssign};
use starlane_resources::data::{BinSrc, DataSet, Meta};
use starlane_resources::message::Fail;

use starlane_resources::ConfigSrc;
use crate::artifact::ArtifactRef;
use crate::cache::ArtifactItem;
use crate::config::app::AppConfig;
use crate::error::Error;
use crate::resource::{ArtifactKind, ResourceAddress, ResourceKey, ResourceType};
use crate::resource::create_args::{artifact_bundle_address, create_args_artifact_bundle, space_address};
use crate::star::core::resource::host::Host;
use crate::star::core::resource::state::StateStore;
use crate::star::StarSkel;
use crate::starlane::api::{AppApi, MechtronApi, StarlaneApi};

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
    ) -> Result<DataSet<BinSrc>, Error> {
        match assign.state_src {
            AssignResourceStateSrc::Direct(data) => return Err("App cannot be stateful".into()),
            AssignResourceStateSrc::Stateless => {
            }
            AssignResourceStateSrc::CreateArgs(args) => {
                return Err("App doesn't currently accept command line args.".into())
            }
        }

        let app_config_artifact = match &assign.stub.archetype.config {
            None => return Err("App requires a config".into() ),
            Some(ConfigSrc::Artifact(artifact)) => {
println!("artifact : {}", artifact.to_string());
                artifact.clone()
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

    async fn init(&self,
                  key: ResourceKey,
    ) -> Result<(),Error> {
println!("CREATE APP create()");
        if key.resource_type() != ResourceType::App {
            return Err("expected AppHost.init() ResourceType to be ResourceType::App".into());
        }
        let record = self.skel.resource_locator_api.locate(key.into() ).await?;
        if let Option::Some(ConfigSrc::Artifact(app_config_artifact)) = record.stub.archetype.config.clone() {
            let factory = self.skel.machine.get_proto_artifact_caches_factory().await?;
            let mut proto = factory.create();
            let app_config_artifact_ref = ArtifactRef::new(app_config_artifact.clone(), ArtifactKind::AppConfig );
            proto.cache(vec![app_config_artifact_ref]).await?;
            let caches = proto.to_caches().await?;
            let app_config = caches.app_configs.get(&app_config_artifact).ok_or::<Error>(format!("expected app_config").into())?;
println!("SO FAR SO GOOD");
            let app_api = AppApi::new( self.skel.surface_api.clone(), record.stub.clone() )?;
            match app_api.create_mechtron("main", app_config.main.address.clone() )?.submit().await {
                Ok(_) => {}
                Err(err) => {
                    eprintln!("potential non-fatal error when creating mechtron: {}", err.to_string());
                }
            }
println!("MECHTRON CREATED");

        } else {
            return Err("expected App to have an artifact for a ConfigSrc".into())
        }

        Ok(())
    }

    async fn has(&self, key: ResourceKey) -> bool {
        match self.store.has(key).await {
            Ok(v) => v,
            Err(_) => false,
        }
    }

    async fn get(&self, key: ResourceKey) -> Result<Option<DataSet<BinSrc>>, Error> {
        self.store.get(key).await
    }

    async fn delete(&self, _identifier: ResourceKey) -> Result<(), Error> {
        unimplemented!()
    }
}

impl AppHost {
    async fn create_from_args(&self, args: String) -> Result<DataSet<BinSrc>,Error> {
        unimplemented!();
    }
}