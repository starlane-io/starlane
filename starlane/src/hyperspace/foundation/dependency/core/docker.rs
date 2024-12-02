use std::collections::HashMap;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use crate::hyperspace::foundation::config;
use crate::hyperspace::foundation::config::{ ProviderConfig};
use crate::hyperspace::foundation::dependency::core::postgres::PostgresClusterCoreConfig;
use crate::hyperspace::foundation::err::FoundationErr;
use crate::hyperspace::foundation::implementation::docker_desktop_foundation::DependencyConfig;
use crate::hyperspace::foundation::kind::{DependencyKind, Kind, ProviderKind};
use crate::hyperspace::foundation::util::Map;
use crate::space::parse::CamelCase;


static REQUIRED: Lazy<Vec<Kind>> = Lazy::new(|| {
    let mut rtn = vec![];

    rtn
});

#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct DockerDaemonConfig {
    pub image: String,
}


impl DockerDaemonConfig {
    pub fn create(config: Map) -> Result<Box<Self>, FoundationErr> {
        let core = PostgresClusterCoreConfig::create(config.clone())?;
        let image= config.from_field("image" )?;

        let providers = config.parse_same("providers" )?;

        Ok(Box::new(DockerDaemonConfig {
            core,
            image,
            providers,
        }))
    }
}

impl DependencyConfig for DockerDaemonConfig {
    fn image(&self) -> &String {
        &self.image
    }
}

impl config::DependencyConfig for DockerDaemonConfig {
    fn kind(&self) -> &DependencyKind {
        &DependencyKind::DockerDaemon
    }

    fn volumes(&self) -> HashMap<String, String> {
        Default::default()
    }

    fn require(&self) -> &Vec<Kind> {
        &REQUIRED
    }

    fn providers<P>(&self) -> &HashMap<CamelCase, P>
    where
        P: ProviderConfig
    {
        todo!()
    }

    fn provider<P>(&self, kind: &ProviderKind) -> Option<&P>
    where
        P: ProviderConfig
    {
        todo!()
    }


    fn clone_me(&self) -> Box<dyn DependencyConfig> {
       Box::new(self.clone())
    }
}


#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct DockerProviderConfig  {
    kind: CamelCase
}
