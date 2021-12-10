use crate::resource::{Kind, ResourceAddress, ArtifactKind};
use crate::artifact::ArtifactRef;
use crate::cache::{Cacheable, Data};
use crate::resource::config::{ResourceConfig, Parser};
use crate::config::bind::yaml::BindConfigYaml;
use std::sync::Arc;
use crate::error::Error;
use std::str::FromStr;
use std::convert::TryInto;
use starlane_resources::ResourcePath;

pub struct BindConfig {
    pub artifact: ResourcePath,
    pub message: Message
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

pub struct Message {
    pub inbound: Inbound
}

pub struct Inbound {
    pub ports: Vec<Port>
}

pub struct Port {
    pub name: String,
    pub payload: Payload
}

pub struct Payload {
    pub aspects: Vec<Aspect>
}

pub struct Aspect {
    pub name: String,
    pub schema: Schema
}


pub enum Schema {
    HttpRequest
}

impl FromStr for Schema {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "http_request" => Ok(Schema::HttpRequest),
            _ => Err(format!("not a known data schema {}",s).into())
        }
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

        let address = artifact.address.clone();
        let bundle_address = address.parent().ok_or::<Error>("expected artifact to have bundle parent".into())?;

        // validate
        for p in &yaml.spec.message.inbound.ports {
            for a in &p.payload.aspects {
                Schema::from_str(a.schema.kind.as_str() )?;
            }
        }

        Ok(Arc::new(BindConfig {
            artifact: artifact.address,
            message: Message {
                inbound: Inbound {
                    ports: yaml.spec.message.inbound.ports.iter().map( |p| Port {
                        name: p.name.clone(),
                        payload: Payload {
                            aspects: p.payload.aspects.iter().map( |a| Aspect{
                                name: a.name.clone(),
                                schema: Schema::from_str(a.schema.kind.as_str()).expect("expected valid schema kind")
                            }).collect()
                        }
                    }).collect()
                }
            }
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
        pub schema: SchemaYaml,
    }

    #[derive(Clone, Serialize, Deserialize)]
    pub struct SchemaYaml{
        pub kind: String,
        pub artifact: Option<String>
    }






}


