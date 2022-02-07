use std::collections::HashMap;

use crate::artifact::ArtifactRef;
use crate::error::Error;
use crate::mechtron::MechtronShell;
use crate::resource::{ArtifactKind, ResourceType, ResourceAssign, AssignResourceStateSrc, Kind};
use crate::star::core::resource::manager::ResourceManager;
use crate::star::core::resource::state::StateStore;
use crate::star::StarSkel;
use crate::util::AsyncHashMap;
use crate::message::delivery::Delivery;
use mesh_portal_serde::version::latest::command::common::StateSrc;
use mesh_portal_serde::version::latest::id::Address;
use mesh_portal_serde::version::latest::messaging::Request;
use mesh_portal_serde::version::latest::payload::{Payload, PayloadPattern, Primitive};
use mesh_portal_serde::version::latest::resource::Properties;
use mesh_portal_versions::version::v0_0_1::pattern::consume_data_struct_def;
use mesh_portal_versions::version::v0_0_1::util::ValueMatcher;

use crate::fail::Fail;
use crate::message::Reply;

lazy_static!{

static ref MECHTRON_PROPERTIES_PATTERN : PayloadPattern= {
             let (_,payload_pattern) = consume_data_struct_def("Map{config<Address>}" ).expect("could not parse PayloadPattern");
             payload_pattern
        };
}


pub struct MechtronManager {
    skel: StarSkel,
    mechtrons: AsyncHashMap<Address, MechtronShell>
}

impl MechtronManager {
    pub async fn new(skel: StarSkel) -> Self {
        MechtronManager {
            skel: skel.clone(),
            mechtrons: AsyncHashMap::new()
        }
    }
}

#[async_trait]
impl ResourceManager for MechtronManager {
    async fn assign(
        &self,
        assign: ResourceAssign,
    ) -> Result<(), Error> {
        match assign.state {
            StateSrc::Stateless => {}
            _ => {
                return Err("currently only supporting stateless mechtrons".into());
            }
        };

        let properties = MechtronProperties::new(assign.stub.properties.clone())?;
        let mechtron_config_artifact = properties.config().ok_or("expected mechtron config")?;

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

    fn handle_request(&self, delivery: Delivery<Request>) {
        unimplemented!()
    }

    fn resource_type(&self) -> ResourceType {
        ResourceType::Mechtron
    }
}

pub struct MechtronProperties {
    properties: Properties
}




impl MechtronProperties {

    pub fn new( properties: Properties ) -> Result<Self,Error> {

        // there's got to be a better way to do this --
        MECHTRON_PROPERTIES_PATTERN.is_match(&properties.clone().into())?;

        Ok(Self{
            properties
        })
    }

    pub fn config(&self) -> Option<Address> {
        match self.properties.get("config") {
            None => {None}
            Some(config) => {
                match config {
                    Payload::Primitive(config) => {
                        match config {
                            Primitive::Address(address) => {
                                Option::Some(address.clone())
                            }
                            _ => {None}
                        }
                    }
                    _ => {None}
                }
            }
        }
    }

}
