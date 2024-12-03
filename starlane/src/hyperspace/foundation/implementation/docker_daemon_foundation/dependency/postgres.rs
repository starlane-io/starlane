use crate::hyperspace::foundation::dependency::core::docker::DockerProviderCoreConfig;
use crate::hyperspace::foundation::dependency::core::postgres::PostgresClusterCoreConfig;
use crate::hyperspace::foundation::err::FoundationErr;
use crate::hyperspace::foundation::implementation::docker_daemon_foundation;
use crate::hyperspace::foundation::implementation::docker_daemon_foundation::Foundation;
use crate::hyperspace::foundation::kind::{DependencyKind, Kind, ProviderKind};
use crate::hyperspace::foundation::util::{ IntoSer, Map, SerMap};
use crate::hyperspace::foundation::{config, LiveService, Provider};
use crate::space::parse::{CamelCase, DbCase, VarCase};
use serde::{Deserialize, Serialize, Serializer};
use std::collections::HashMap;
use std::ops::Deref;
use std::str::FromStr;
use std::sync::Arc;

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



impl config::DependencyConfig for PostgresDependencyConfig {
    fn kind(&self) -> &DependencyKind {
        &DependencyKind::PostgresCluster
    }

    fn volumes(&self) -> HashMap<String, String> {
        self.postgres.volumes()
    }

    fn require(&self) -> Vec<Kind> {
        self.postgres.require()
    }

    fn clone_me(&self) -> Arc<dyn config::DependencyConfig> {
        Arc::new(self.clone()) as Arc<dyn config::DependencyConfig>
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
