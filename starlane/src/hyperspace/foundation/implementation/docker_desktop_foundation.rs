use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use ascii::AsciiChar::k;
use serde_yaml::Value;
use crate::hyperspace::foundation::err::FoundationErr;
use crate::hyperspace::foundation::kind::{DependencyKind, FoundationKind, ProviderKind};
use crate::hyperspace::foundation::settings::FoundationSettings;
use crate::hyperspace::foundation::config;use crate::hyperspace::foundation::config::{DependencyConfig};
use crate::hyperspace::foundation::dependency::implementation::docker::DockerDaemonConfig;
use crate::hyperspace::foundation::dependency::implementation::postgres::PostgresClusterConfig;
use crate::hyperspace::foundation::traits;
use crate::hyperspace::foundation::util::Map;

pub mod dependency;


pub struct Foundation {
    config: FoundationConfig
}

impl traits::Foundation for Foundation {

    type Config = FoundationConfig;

    fn create(config: Self::Config) -> Result<impl config::Foundation<Config=Self::Config>, FoundationErr> {
        Ok(Self{
            config
        })
    }

    fn kind() -> FoundationKind {
        FoundationKind::DockerDaemon
    }
}


#[derive(Clone,Debug,Serialize, Deserialize)]
pub struct FoundationConfig{
    pub kind: FoundationKind,
    pub registry: RegistryConfig,
    pub dependencies: HashMap<DependencyKind,dyn DependencyConfig>,
}

impl FoundationConfig {
   pub fn create( config: Map ) -> Result<FoundationConfig, FoundationErr> {
           let kind = config.kind()?;
           let registry = config.from_field("registry")?;

           let dependencies =  config.parse_kinds("dependencies", | map: Map | -> Result<Map,FoundationErr>{
               let kind: DependencyKind = map.kind()?;
               match kind {
                   DependencyKind::PostgresCluster => PostgresClusterConfig::create(map)?,
                   DependencyKind::DockerDaemon => DockerDaemonConfig::create(map)?,
               }
           })?;

           Ok(Self {
               kind,
               registry,
               dependencies,
           })
       }
   }




impl config::FoundationConfig for FoundationConfig{
    fn kind(&self) -> & FoundationKind {
        & self.kind
    }

    fn dependency_kinds(&self) -> Vec<&'static str> {
        self.dependencies.keys().map(|kind| kind.clone()).collect()
    }

    fn dependency(&self, kind: &DependencyKind) -> Option<&impl config::DependencyConfig> {
        self.dependencies.get(kind)
    }

    fn create_dependencies(&self, deps: Vec<Value>) -> Result<impl config::DependencyConfig, FoundationErr> {
        todo!()
    }
}



#[derive(Clone,Debug,Serialize, Deserialize)]
pub struct RegistryConfig {
    provider: ProviderKind,
}









