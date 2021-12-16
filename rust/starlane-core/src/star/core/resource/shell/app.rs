use std::convert::TryInto;
use std::sync::Arc;

use clap::{App, AppSettings};
use yaml_rust::Yaml;

use crate::artifact::ArtifactRef;
use crate::cache::ArtifactItem;
use crate::config::app::AppConfig;
use crate::error::Error;
use crate::resource::{ArtifactKind, ResourceType, ResourceAssign, AssignResourceStateSrc};
use crate::star::core::resource::shell::Host;
use crate::star::core::resource::state::StateStore;
use crate::star::StarSkel;
use std::collections::HashMap;
use crate::util::AsyncHashMap;
use mesh_portal_serde::version::latest::resource::Status;
use crate::message::delivery::Delivery;
use mesh_portal_api::message::Message;
use crate::mesh::serde::resource::command::common::StateSrc;
use crate::mesh::Request;
use crate::mesh::serde::id::Address;

pub struct AppHost {
    skel: StarSkel,
    apps: AsyncHashMap<Address,Status>
}

impl AppHost {
    pub async fn new(skel: StarSkel) -> Self {
        AppHost {
            skel: skel.clone(),
            apps: AsyncHashMap::new()
        }
    }
}

#[async_trait]
impl Host for AppHost {
    fn resource_type(&self) -> ResourceType {
        ResourceType::App
    }

    async fn assign(
        &self,
        assign: ResourceAssign,
    ) -> Result<(), Error> {
        match assign.state {
            StateSrc::StatefulDirect(data) => return Err("App cannot be stateful".into()),
            StateSrc::Stateless => {
            }
        }

        unimplemented!()

        /*

        let factory = self.skel.machine.get_proto_artifact_caches_factory().await?;
        let mut proto = factory.create();
        let app_config_artifact_ref = ArtifactRef::new(app_config_artifact.clone(), ArtifactKind::AppConfig );
        proto.cache(vec![app_config_artifact_ref]).await?;
        let caches = proto.to_caches().await?;

        println!("App config loaded!");

        self.apps.put( assign.stub.key.clone(), Status::Ready ).await;

        Ok(())

         */
    }

    fn request(&self, request : Delivery<Request>) {
        todo!()
    }

    /*
    async fn init(&self,
                  key: Address,
    ) -> Result<(),Error> {
println!("CREATE APP create()");
        if key.resource_type() != ResourceType::App {
            return Err("expected AppHost.init() ResourceType to be ResourceType::App".into());
        }
        let record = self.skel.resource_locator_api.locate(key.into() ).await?;
        if let ConfigSrc::Artifact(app_config_artifact) = record.stub.archetype.config.clone() {
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
    }*/

    async fn has(&self, key: Address) -> bool {
        self.apps.contains( key ).await.unwrap_or(false)
    }

}

