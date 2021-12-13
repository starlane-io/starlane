use std::collections::HashMap;

use crate::artifact::ArtifactRef;
use crate::error::Error;
use crate::mechtron::MechtronShell;
use crate::resource::{ArtifactKind,  ResourceType, ResourceAssign, AssignResourceStateSrc};
use crate::star::core::resource::host::Host;
use crate::star::core::resource::state::StateStore;
use crate::star::StarSkel;
use crate::util::AsyncHashMap;
use crate::message::delivery::Delivery;
use crate::mesh::serde::payload::Payload;
use crate::mesh::serde::entity::request::{Msg, Http};
use mesh_portal_api::message::Message;
use mesh_portal_api_client::PortalCtrl;
use crate::mesh::serde::id::Address;

pub struct MechtronHost {
    skel: StarSkel,
    mechtrons: AsyncHashMap<Address, MechtronShell>

}

impl MechtronHost {
    pub async fn new(skel: StarSkel) -> Self {
        MechtronHost {
            skel: skel.clone(),
            mechtrons: AsyncHashMap::new()
        }
    }
}

#[async_trait]
impl Host for MechtronHost {
    async fn assign(
        &self,
        assign: ResourceAssign<AssignResourceStateSrc>,
    ) -> Result<Payload, Error> {
        match assign.state_src {
            AssignResourceStateSrc::Stateless => {}
            _ => {
                return Err("currently only supporting stateless mechtrons".into());
            }
        };

        let mechtron_config_artifact = match &assign.stub.archetype.config {
            ConfigSrc::None => return Err("Mechtron requires a config".into() ),
            ConfigSrc::Artifact(artifact) => {
                println!("artifact : {}", artifact.to_string());
                artifact.clone()
            }
            _ => return Err("Mechtron requires a config referencing an artifact".into() ),
        };

        let factory = self.skel.machine.get_proto_artifact_caches_factory().await?;
        let mut proto = factory.create();
        let mechtron_config_artifact_ref = ArtifactRef::new(mechtron_config_artifact.clone(), ArtifactKind::MechtronConfig );
        proto.cache(vec![mechtron_config_artifact_ref]).await?;
        let caches = proto.to_caches().await?;
        let mechtron_config = caches.mechtron_configs.get(&mechtron_config_artifact).ok_or::<Error>(format!("expected mechtron_config").into())?;


        unimplemented!();
        /*
        let mechtron = MechtronShell::new(mechtron_config, &caches)?;
        self.mechtrons.put( assign.stub.key.clone(), mechtron ).await?;

        println!("ASSIGN MECHTRON!");


        Ok(DataSet::new())

         */
    }

    async fn has(&self, address: Address) -> bool {
        match self.mechtrons.contains(address).await {
            Ok(flag) => {flag}
            Err(_) => {false}
        }
    }

    fn handle(&self,  delivery: Delivery<Message>) -> Result<(),Error>{

        tokio::spawn( async move {
            info!("MECHTRON HOST RECEIVED DELIVERY");
            let mechtron = self.mechtrons.get(key.clone()).await?.ok_or(format!("could not deliver mechtron to {}", key.to_string()))?;
            info!("GOT MECHTRON");
            let reply = mechtron.handle(delivery.request.clone()).await?;

            if let Option::Some(reply) = reply {
                delivery.reply(Reply::Port(reply.payload));
                info!("=====>> MECHTRON SENT REPLY");
            }
        });

        Ok(())
    }

    fn resource_type(&self) -> ResourceType {
        ResourceType::Mechtron
    }
}
