use crate::base::common::postgres::PostgresClusterCoreConfig;
use crate::base::foundation::err::FoundationErr;
use crate::base::foundation::implementation::docker_daemon_foundation;
use crate::base::foundation::util::{IntoSer, Map, SerMap};
use crate::base::foundation::Provider;
use crate::space::parse::{CamelCase, DbCase};
use serde::{Deserialize, Serialize, Serializer};
use std::collections::HashMap;
use std::ops::Deref;
use std::str::FromStr;
use crate::base;
use crate::base::kind::{DependencyKind, Kind, ProviderKind};
use crate::base::foundation::config;

fn default_schema() -> DbCase {
    DbCase::from_str("PUBLIC").unwrap()
}

fn default_registry_database() -> DbCase {
    DbCase::from_str("REGISTRY").unwrap()
}

fn default_registry_provider_kind() -> CamelCase {
    CamelCase::from_str("Registry").unwrap()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostgresDependencyConfig {
    pub postgres: PostgresClusterCoreConfig,
    pub docker: ProviderKind,
    pub image: String,
}

impl PostgresDependencyConfig {
    pub fn create(config: Map) -> Result<Self, FoundationErr> {
        let postgres = PostgresClusterCoreConfig::create(config.clone())?;
        let docker = config.from_field("docker")?;
        let docker = ProviderKind::new(DependencyKind::DockerDaemon, docker);
        let image = config.from_field("image")?;
        Ok(Self {
            postgres,
            docker,
            image,
        })
    }
}

impl Deref for PostgresDependencyConfig {
    type Target = PostgresClusterCoreConfig;

    fn deref(&self) -> &Self::Target {
        &self.postgres
    }
}

pub trait ProviderConfig: base::config::ProviderConfig {

}


impl base::config::DependencyConfig for PostgresDependencyConfig {
    type ProviderConfig = ;

    fn kind(&self) -> &DependencyKind {
        todo!()
    }

    fn require(&self) -> Vec<Kind> {
        todo!()
    }
}

impl config::DependencyConfig for PostgresDependencyConfig where Self::ProviderConfig:  PostgresDependencyConfig

  pub type Depe=PostgresDependencyConfig;

    fn volumes(&self) -> HashMap<String, String> {
        self.postgres.volumes()
    }

}

impl IntoSer for PostgresDependencyConfig {
    fn into_ser(&self) -> Box<dyn SerMap> {
        Box::new(self.clone()) as Box<dyn SerMap>
    }
}

impl docker_daemon_foundation::DependencyConfig for PostgresDependencyConfig {
    fn image(&self) -> String {
        self.image.clone()
    }
}
