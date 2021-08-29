use std::collections::HashMap;

use starlane_resources::{AssignResourceStateSrc, Resource, ResourceAssign};
use starlane_resources::data::{BinSrc, DataSet};
use starlane_resources::message::{Fail, ResourcePortMessage, Message};

use starlane_resources::ConfigSrc;
use crate::artifact::ArtifactRef;
use crate::error::Error;
use crate::mechtron::Mechtron;
use crate::resource::{ArtifactKind, ResourceKey};
use crate::star::core::resource::host::Host;
use crate::star::core::resource::state::StateStore;
use crate::star::StarSkel;
use crate::util::AsyncHashMap;
use crate::message::resource::Delivery;

pub struct MechtronHost {
    skel: StarSkel,
    mechtrons: AsyncHashMap<ResourceKey,Mechtron>

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
        assign: ResourceAssign<AssignResourceStateSrc<DataSet<BinSrc>>>,
    ) -> Result<DataSet<BinSrc>, Error> {
        match assign.state_src {
            AssignResourceStateSrc::Stateless => {}
            _ => {
                return Err("currently only supporting stateless mechtrons".into());
            }
        };

        let mechtron_config_artifact = match &assign.stub.archetype.config {
            None => return Err("Mechtron requires a config".into() ),
            Some(ConfigSrc::Artifact(artifact)) => {
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


        let mechtron = Mechtron::new(mechtron_config, &caches)?;
        self.mechtrons.put( assign.stub.key.clone(), mechtron ).await?;

        println!("ASSIGN MECHTRON!");


        Ok(DataSet::new())
    }

    async fn has(&self, key: ResourceKey) -> bool {
        match self.mechtrons.contains(key).await {
            Ok(flag) => {flag}
            Err(_) => {false}
        }
    }

    async fn get(&self, key: ResourceKey) -> Result<Option<DataSet<BinSrc>>, Error> {
        // since we only support stateless for now
        Ok(Option::None)
    }

    async fn delete(&self, _identifier: ResourceKey) -> Result<(), Error> {
        unimplemented!()
    }

    async fn deliver(&self, key: ResourceKey, delivery: Delivery<Message<ResourcePortMessage>>) -> Result<(),Error>{

        info!("MECHTRON HOST RECEIVED DELIVERY");
        let mechtron = self.mechtrons.get(key.clone()).await?.ok_or(format!("could not deliver mechtron to {}",key.to_string()))?;
        info!("GOT MECHTRON");

        Ok(())
    }

}
