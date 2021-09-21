use crate::resource::{ResourceKind, ResourceAddress};
use crate::cache::{Cacheable, Data};
use std::collections::HashMap;
use crate::resource::ArtifactKind;
use crate::artifact::ArtifactRef;
use crate::resource::config::{ResourceConfig, Parser};
use std::sync::Arc;
use crate::error::Error;
use crate::config::app::yaml::AppConfigYaml;
use std::str::FromStr;
use std::convert::TryInto;
use starlane_resources::ResourcePath;

pub struct AppConfig {
    artifact: ResourcePath,
    pub main: ArtifactRef
}

impl Cacheable for AppConfig {
    fn artifact(&self) -> ArtifactRef {
        ArtifactRef {
            path: self.artifact.clone(),
            kind: ArtifactKind::AppConfig,
        }
    }

    fn references(&self) -> Vec<ArtifactRef> {
        vec![]
    }
}

impl ResourceConfig for AppConfigParser {
    fn kind(&self) -> ResourceKind {
        ResourceKind::App
    }
}

pub struct AppConfigParser;

impl AppConfigParser {
    pub fn new() -> Self {
        Self {}
    }
}

impl Parser<AppConfig> for AppConfigParser {
    fn parse(&self, artifact: ArtifactRef, _data: Data) -> Result<Arc<AppConfig>, Error> {

        let data = String::from_utf8((*_data).clone() )?;
        let yaml: AppConfigYaml = serde_yaml::from_str( data.as_str() )?;

        let address: ResourcePath = artifact.path.clone();
        let bundle_address = address.parent().ok_or::<Error>("expected artifact to have bundle parent".into())?;

        let main = yaml.spec.main.replace("{bundle}", bundle_address.to_string().as_str() );
        let main = ResourcePath::from_str(main.as_str() )?;
        let main = ArtifactRef::new(main.try_into()?,ArtifactKind::MechtronConfig );

        Ok(Arc::new(AppConfig {
            artifact: artifact.path,
            main
        }))
    }
}

mod yaml {
    use serde::{Serialize,Deserialize};

    #[derive(Clone, Serialize, Deserialize)]
    pub struct AppConfigYaml {
        pub kind: String,
        pub spec: SpecYaml
    }

    #[derive(Clone, Serialize, Deserialize)]
    pub struct SpecYaml {
        pub main: String
    }
}


