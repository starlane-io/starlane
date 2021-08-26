use crate::resource::{ArtifactAddress, ResourceKind, ResourceAddress,ArtifactKind};
use crate::artifact::ArtifactRef;
use crate::cache::{Cacheable, Data};
use crate::resource::config::{ResourceConfig, Parser};
use crate::config::bind::yaml::BindConfigYaml;
use std::sync::Arc;
use crate::error::Error;
use std::str::FromStr;
use std::convert::TryInto;

pub struct BindConfig {
    pub artifact: ArtifactAddress,
}

impl Cacheable for BindConfig {
    fn artifact(&self) -> ArtifactRef {
        ArtifactRef {
            address: self.artifact.clone(),
            kind: ArtifactKind::BindConfig
        }
    }

    fn references(&self) -> Vec<ArtifactRef> {
        vec![]
    }
}


pub struct BindConfigParser;

impl BindConfigParser {
    pub fn new() -> Self {
        Self {}
    }
}

impl Parser<BindConfig> for BindConfigParser {
    fn parse(&self, artifact: ArtifactRef, _data: Data) -> Result<Arc<BindConfig>, Error> {

        let data = String::from_utf8((*_data).clone() )?;
        let yaml: BindConfigYaml = serde_yaml::from_str( data.as_str() )?;

        let address: ResourceAddress  = artifact.address.clone().into();
        let bundle_address = address.parent().ok_or::<Error>("expected artifact to have bundle parent".into())?;

        Ok(Arc::new(BindConfig {
            artifact: artifact.address,

        }))
    }
}

mod yaml {
    use serde::{Serialize,Deserialize};

    #[derive(Clone, Serialize, Deserialize)]
    pub struct BindConfigYaml {
        pub kind: String,
        pub spec: SpecYaml
    }

    #[derive(Clone, Serialize, Deserialize)]
    pub struct SpecYaml {
        pub name: String,
        pub state: StateYaml,
        pub message: MessageYaml
    }

    #[derive(Clone, Serialize, Deserialize)]
    pub struct StateYaml{
        pub kind: StateKindYaml
    }

    #[derive(Clone, Serialize, Deserialize)]
    pub enum StateKindYaml{
        Stateless
    }


    #[derive(Clone, Serialize, Deserialize)]
    pub struct MessageYaml{
        pub inbound: MessageInboundYaml
    }

    #[derive(Clone, Serialize, Deserialize)]
    pub struct MessageInboundYaml{
        pub ports: Vec<PortsYaml>
   }

    #[derive(Clone, Serialize, Deserialize)]
    pub struct PortsYaml{
        pub name: String,
        pub payload: PayloadYaml
    }

    #[derive(Clone, Serialize, Deserialize)]
    pub struct PayloadYaml{
        pub aspects: Vec<AspectYaml>
    }


    #[derive(Clone, Serialize, Deserialize)]
    pub struct AspectYaml{
        pub name: String,
        pub schema: String,
        pub artifact: String
    }




}


