use std::any::Any;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::str::FromStr;
use std::sync::Arc;
use derive_name::{Name, Named};
use once_cell::sync::Lazy;
use serde::de::DeserializeOwned;
use tokio::sync::watch::Receiver;
use crate::hyperspace::foundation;
use crate::hyperspace::foundation::{config, Dependency};
use crate::hyperspace::foundation::config::ConfigMap;
use crate::hyperspace::foundation::err::FoundationErr;
use crate::hyperspace::foundation::kind::{DependencyKind, FoundationKind, IKind, Kind, ProviderKind};
use crate::hyperspace::foundation::dependency::core::docker::DockerDaemonCoreDependencyConfig;
use crate::hyperspace::foundation::dependency::core::postgres::PostgresClusterCoreConfig;
use crate::hyperspace::foundation::status::Status;
use crate::hyperspace::foundation::util::{DesMap, IntoSer, Map, SerMap};
use crate::hyperspace::reg::Registry;
use crate::space::parse::CamelCase;
use crate::space::progress::Progress;

pub mod dependency;

/// the REQUIRED
static REQUIRED: Lazy<Vec<Kind>> = Lazy::new(|| {
    /// we obviously need DockerDaemon to be installed and running or this Foundation can't do a thing
    let docker_daemon = DependencyKind::DockerDaemon.into();
    /// and this Foundation supports a Postgres Registry (Also very important)
    let registry = ProviderKind::new(DependencyKind::PostgresCluster,CamelCase::from_str("Registry").unwrap()).into();
    let mut rtn = vec![docker_daemon, registry];

    rtn
});

/// this method is referenced by various [`DependencyConfig`] as a Default value (which in the case of [`FoundationKind::DockerDaemon`] every [`Dependency`]
/// and [`Provisioner`] requires the Docker Daemon to be installed and running
pub fn default_requirements() -> Vec<Kind> {
    REQUIRED.clone()
}

pub struct Foundation {
    config: Arc<FoundationConfig>,
    status: Arc<tokio::sync::watch::Receiver<Status>>,
    status_tx: tokio::sync::watch::Sender<Status>,
}

impl Foundation {

    pub fn new(config: FoundationConfig) -> Self {
        let (status_tx,status) = tokio::sync::watch::channel(Status::default());
        let status = Arc::new(status);
        let config = Arc::new(config);
        Self {
            config,
            status,
            status_tx
        }
    }
}

#[async_trait]
impl foundation::Foundation for Foundation {
    fn kind(&self) -> &FoundationKind{
        &FoundationKind::DockerDaemon
    }

    fn config(&self) -> Arc<dyn config::FoundationConfig>{
        let config: Arc<dyn config::FoundationConfig> = self.config.clone();
        config
    }

    fn status(&self) -> Status {
        self.status.borrow().clone()
    }

    fn status_watcher(&self) -> Arc<Receiver<Status>> {
        self.status.clone()
    }

    async fn synchronize(&self, progress: Progress) -> Result<Status,FoundationErr> {
        todo!();
        Ok(Default::default())
    }

    /// Ensure that core dependencies are downloaded, installed and initialized
    /// in the case of [`FoundationKind::DockerDaemon`] we first check if the Docker Daemon
    /// is installed and running.  This installer does not actually install DockerDaemonb
    async fn install(&self, progress: Progress) -> Result<(), FoundationErr> {
        Ok(())
    }

    fn dependency(&self, kind: &DependencyKind) -> Result<Option<Box<dyn Dependency>>, FoundationErr> {
        todo!()
    }

    fn registry(&self) -> Result<Registry,FoundationErr> {
        todo!()
    }
}


#[derive(Clone,Serialize, Deserialize,Name)]
pub struct FoundationConfig {
    pub kind: FoundationKind,
    pub registry: RegistryProviderConfig,
    pub dependencies: ConfigMap<DependencyKind,Arc<dyn DependencyConfig>>,
}





impl FoundationConfig {

    /*
   pub(self) fn des_from_map(map: impl SerMap) -> Result<Self, FoundationErr> {
           let map = map.to_map()?;
           let kind = map.kind()?;

           let registry = map.from_field("registry")?;

           let dependencies: Map  = map.from_field_opt("dependencies")?.unwrap_or_else(|| Default::default());
           let dependencies = dependencies.to_config_map(Self::des_dependency_factory)?;

           Ok(Self {
               kind,
               registry,
               dependencies,
           })
       }


    pub(self) fn des_dependency_factory(dependency: impl SerMap) -> Result<Arc<dyn DependencyConfig>, FoundationErr> {
        let map = dependency.to_map()?;
        let kind: DependencyKind = map.kind()?;

        match kind {
            DependencyKind::PostgresCluster => PostgresClusterCoreConfig::create_as_trait(map),
            DependencyKind::DockerDaemon => DockerDaemonCoreDependencyConfig::create_as_trait(map),
        }
    }

    pub(self) fn ser_dependencies(dependencies: HashMap<DependencyKind, Arc<impl DependencyConfig+SerMap>>)  -> Result<serde_yaml::Value,FoundationErr> {
        let mut sequence = serde_yaml::Sequence::new();
        for (_,item) in dependencies.into_iter() {
            sequence.push( (*item).clone().to_value()?);
        }
        sequence.to_value()
    }

     */


}


pub trait DependencyConfig: config::DependencyConfig {
    fn image(&self) -> String;

}



impl config::FoundationConfig for FoundationConfig {
    fn kind(&self) -> & FoundationKind {
        & self.kind
    }

    fn required(&self) -> &Vec<Kind> {
        &default_requirements()
    }

    fn dependency_kinds(&self) -> &Vec<DependencyKind> {
        self.dependencies.keys().collect()
    }

    fn dependency(&self, kind: &DependencyKind) -> Option<&Arc<dyn config::DependencyConfig>> {
        self.dependencies.get(kind)
    }

    fn clone_me(&self) -> Arc<dyn config::FoundationConfig>{
       Arc::new(self.clone())
    }
}


impl IntoSer for FoundationConfig {
    fn into_ser(&self) -> Box<dyn SerMap> {
        self.clone() as Box<dyn SerMap>
    }
}

#[derive(Clone,Debug,Serialize, Deserialize)]
pub struct RegistryProviderConfig {
    provider: ProviderKind,
}









#[cfg(test)]
pub mod test {
    use crate::hyperspace::foundation::err::FoundationErr;
    use crate::hyperspace::foundation::implementation::docker_daemon_foundation::FoundationConfig;
    #[test]
    pub fn foundation_config() {
        fn inner() -> Result<(), FoundationErr> {
            let foundation_config = include_str!("docker-daemon-test-config.yaml");
            let foundation_config = serde_yaml::from_str( foundation_config ).map_err(FoundationErr::config_err)?;
            let foundation_config = FoundationConfig::des_from_map(foundation_config)?;

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

