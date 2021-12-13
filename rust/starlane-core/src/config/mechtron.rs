use crate::resource::{Kind, ArtifactKind};
use crate::artifact::ArtifactRef;
use crate::cache::{Cacheable, Data};
use crate::resource::config::{ResourceConfig, Parser};
use crate::config::mechtron::yaml::MechtronConfigYaml;
use std::sync::Arc;
use crate::error::Error;
use std::str::FromStr;
use std::convert::TryInto;
use crate::mesh::serde::id::Address;

pub struct MechtronConfig {
    pub name: String,
    pub prefix: String,
    pub artifact: Address,
    pub bind: ArtifactRef,
    pub wasm: ArtifactRef,
}

impl Cacheable for MechtronConfig {
    fn artifact(&self) -> ArtifactRef {
        ArtifactRef {
            address: self.artifact.clone(),
            kind: ArtifactKind::MechtronConfig,
        }
    }

    fn references(&self) -> Vec<ArtifactRef> {
        vec![self.bind.clone(),self.wasm.clone()]
    }
}

impl ResourceConfig for MechtronConfigParser {
    fn kind(&self) -> Kind {
        Kind::Mechtron
    }
}

pub struct MechtronConfigParser;

impl MechtronConfigParser {
    pub fn new() -> Self {
        Self {}
    }
}

impl Parser<MechtronConfig> for MechtronConfigParser {
    fn parse(&self, artifact: ArtifactRef, _data: Data) -> Result<Arc<MechtronConfig>, Error> {

        let data = String::from_utf8((*_data).clone() )?;
        let yaml: MechtronConfigYaml = serde_yaml::from_str( data.as_str() )?;

        let address = artifact.address.clone();
        let bundle_address = address.parent().ok_or::<Error>("expected artifact to have bundle parent".into())?;

        let bind = yaml.spec.bind.replace("{bundle}", bundle_address.to_string().as_str() );
        let bind= Address::from_str(bind.as_str() )?;
        let bind = ArtifactRef::new(bind.try_into()?,ArtifactKind::BindConfig);

        let wasm = yaml.spec.wasm.replace("{bundle}", bundle_address.to_string().as_str() );
        let wasm= Address::from_str(wasm.as_str() )?;
        let wasm = ArtifactRef::new(wasm.try_into()?,ArtifactKind::Wasm);

        Ok(Arc::new(MechtronConfig {
            artifact: artifact.address,
            bind,
            wasm,
            name: yaml.spec.name,
            prefix: yaml.spec.prefix
        }))
    }
}

mod yaml {
    use serde::{Serialize,Deserialize};

    #[derive(Clone, Serialize, Deserialize)]
    pub struct MechtronConfigYaml {
        pub kind: String,
        pub spec: SpecYaml
    }

    #[derive(Clone, Serialize, Deserialize)]
    pub struct SpecYaml {
        pub name: String,
        pub prefix: String,
        pub bind: String,
        pub wasm: String
    }
}


