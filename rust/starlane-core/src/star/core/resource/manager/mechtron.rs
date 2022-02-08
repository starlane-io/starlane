use std::collections::HashMap;
use std::str::FromStr;

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
use crate::command::cli::outlet;
use crate::command::cli::outlet::Frame;
use crate::command::execute::CommandExecutor;
use crate::config::config::MechtronConfig;
use crate::config::parse::replace::substitute;

use crate::fail::Fail;
use crate::message::Reply;
use crate::starlane::api::StarlaneApi;

pub struct MechtronManager {
    skel: StarSkel,
    mechtrons: AsyncHashMap<Address, MechtronShell>,
    resource_type: ResourceType
}

impl MechtronManager {
    pub async fn new(skel: StarSkel, resource_type:ResourceType) -> Self {
        MechtronManager {
            skel: skel.clone(),
            mechtrons: AsyncHashMap::new(),
            resource_type
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

        let config_address = assign.stub.properties.get(&"config".to_string() ).ok_or(format!("'config' property required to be set for {}", self.resource_type.to_string() ))?.value.as_str();
        let config_address = Address::from_str(config_address)?;

        let config_artifact_ref = ArtifactRef {
          address:config_address.clone(),
          kind: ArtifactKind::ResourceConfig
        };

        let caches = self.skel.machine.cache( &config_artifact_ref ).await?;
        let config = caches.resource_configs.get(&config_address).ok_or::<Error>(format!("expected mechtron_config").into())?;
        let config = MechtronConfig::new(config, assign.stub.address.clone() );

        let api = StarlaneApi::new( self.skel.surface_api.clone(), assign.stub.address.clone() );
        let substitution_map = config.substitution_map()?;
        for mut command_line in config.install {
            command_line = substitute(command_line.as_str(), &substitution_map)?;
            println!("INSTALL: '{}'",command_line);
            let mut output_rx = CommandExecutor::exec_simple(command_line,assign.stub.clone(), api.clone() );
            while let Some(frame) = output_rx.recv().await {
                match frame {
                    outlet::Frame::StdOut(out) => {
                        println!("{}",out);
                    }
                    outlet::Frame::StdErr(out) => {
                        eprintln!("{}", out);
                    }
                    outlet::Frame::EndOfCommand(code) => {
                        if code != 0 {
                            eprintln!("install error code: {}",code);
                        }
                    }
                }
            }
        }


//        let mechtron = MechtronShell::new(mechtron_config, caches)?;
//        self.mechtrons.put( assign.stub.key.clone(), mechtron ).await?;

        Ok(())
    }

    fn handle_request(&self, delivery: Delivery<Request>) {
        unimplemented!()
    }

    fn resource_type(&self) -> ResourceType {
        self.resource_type.clone()
    }
}

