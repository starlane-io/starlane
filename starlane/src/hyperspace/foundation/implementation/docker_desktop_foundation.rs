use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use once_cell::sync::Lazy;
use tokio::sync::watch::Receiver;
use crate::hyperspace::foundation;
use crate::hyperspace::foundation::err::FoundationErr;
use crate::hyperspace::foundation::kind::{DependencyKind, FoundationKind, Kind, ProviderKind};
use crate::hyperspace::foundation::{config, Dependency};use crate::hyperspace::foundation::config::DependencyConfig;
use crate::hyperspace::foundation::dependency::implementation::docker::DockerDaemonConfig;
use crate::hyperspace::foundation::dependency::implementation::postgres::PostgresClusterConfig;
use crate::hyperspace::foundation::status::{Phase, Status, StatusDetail};
use crate::hyperspace::foundation::util::Map;
use crate::hyperspace::reg::Registry;
use crate::space::parse::CamelCase;
use crate::space::progress::Progress;

pub mod dependency;

/// the REQUIRED
static REQUIRED: Lazy<Vec<Kind>> = Lazy::new(|| {
    let docker = DependencyKind::DockerDaemon.into();
    let registry = ProviderKind::new(DependencyKind::PostgresCluster,CamelCase::from_str("Registry").unwrap()).into();
    let mut rtn = vec![docker,registry];

    rtn
});

pub struct Foundation {
    config: FoundationConfig,
    status: Arc<tokio::sync::watch::Receiver<Status>>
}

impl Foundation {

    pub fn new(config: FoundationConfig) -> Self {
        Self {
            config,
            status: Default::default(),
        }
    }


    /// this method is referenced by various [`DependencyConfig`] as a Default value (which in the case of [`FoundationKind::DockerDaemon`] every [`Dependency`]
    /// and [`Provisioner`] requires the Docker Daemon to be installed and running
    pub fn default_requirements() -> Vec<Kind> {
        vec![DependencyKind::DockerDaemon.into()]
    }
}

impl foundation::Foundation for Foundation {
    fn kind(&self) -> &FoundationKind{
        &FoundationKind::DockerDaemon
    }

    fn config(&self) -> &impl config::FoundationConfig {
        &self.config
    }


    fn status(&self) -> Status{
        self.status.borrow().clone()
    }

    fn status_watcher(&self) -> Arc<Receiver<Status>> {
        self.status.clone()
    }

    async fn synchronize(&self, progress: Progress) -> Result<(), FoundationErr> {
        Ok(())
    }


    /// Ensure that core dependencies are downloaded, installed and initialized
    /// in the case of [`FoundationKind::DockerDaemon`] we first check if the Docker Daemon
    /// is installed and running.  This installer does not actually install DockerDaemonb
    async fn install(&self, progress: Progress) -> Result<(), FoundationErr> {
        Ok(())
    }

    fn dependency(&self, kind: &DependencyKind) -> Result<Option<impl Dependency>, FoundationErr> {
        todo!()
    }

    fn registry(&self) -> Result<&Registry, FoundationErr> {
        todo!()
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

    fn required(&self) -> &Vec<Kind> {
        &Foundation::default_requirements()
    }

    fn dependency_kinds(&self) -> Vec<&'static str> {
        self.dependencies.keys().map(|kind| kind.clone()).collect()
    }

    fn dependency(&self, kind: &DependencyKind) -> Option<&impl config::DependencyConfig> {
        self.dependencies.get(kind)
    }

    fn clone_me(&self) -> Box<dyn config::FoundationConfig>{
       Box::new(self.clone())
    }
}



#[derive(Clone,Debug,Serialize, Deserialize)]
pub struct RegistryConfig {
    provider: ProviderKind,
}




#[cfg(test)]
pub mod test {
    use crate::hyperspace::foundation::err::FoundationErr;
    use crate::hyperspace::foundation::implementation::docker_desktop_foundation::{Foundation, FoundationConfig};
    use crate::hyperspace::foundation::util::Map;

    #[test]
    pub fn foundation_config() {
        fn inner() -> Result<(), FoundationErr> {
            let foundation_config = include_str!("docker-daemon-test-config.yaml");
            let foundation_config = serde_yaml::from_str( foundation_config ).map_err(FoundationErr::config_err)?;
            let foundation_config = FoundationConfig::create(foundation_config)?;

            Ok(())
        }

        match inner() {
            Ok(_) => {}
            Err(err) => {
                println!("ERR: {}", err);
                Err::<(),FoundationErr>(err).unwrap();
                assert!(false)
            }
        }
    }
}

