use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use serde_yaml::Value;
use crate::hyperspace::foundation;
use crate::hyperspace::foundation::err::FoundationErr;
use crate::hyperspace::foundation::kind::{DependencyKind, FoundationKind, ProviderKind};
use crate::hyperspace::foundation::config;use crate::hyperspace::foundation::config::DependencyConfig;
use crate::hyperspace::foundation::dependency::implementation::docker::DockerDaemonConfig;
use crate::hyperspace::foundation::dependency::implementation::postgres::PostgresClusterConfig;
use crate::hyperspace::foundation::util::Map;
use crate::hyperspace::reg::Registry;
use crate::space::progress::Progress;

pub mod dependency;


pub struct Foundation {
    config: FoundationConfig
}

impl foundation::Foundation for Foundation {
    fn kind(&self) -> &FoundationKind{
        &FoundationKind::DockerDaemon
    }

    fn config(&self) -> &impl config::FoundationConfig {
        &self.config
    }

    /// Ensure that core dependencies are downloaded, installed and initialized
    /// in the case of [`FoundationKind::DockerDaemon`] we first check if the Docker Daemon
    /// is installed and running.  This installer does not actually install DockerDaemonb
    fn install(&self, progress: Progress) -> Result<(), FoundationErr> {
        Ok(())
    }

    fn registry(&self) -> Result<&Registry, FoundationErr> {
        Err(FoundationErr::FoundationError {kind: self.kind().clone(), })
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









