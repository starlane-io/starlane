use crate::base::err::BaseErr;
use crate::base::foundation::implementation::docker_daemon_foundation;
use crate::base::foundation::util::{IntoSer, Map, SerMap};
use crate::base::foundation::Provider;
use crate::space::parse::{CamelCase, DbCase};
use serde::{Deserialize, Serialize, Serializer};
use std::collections::HashMap;
use std::ops::Deref;
use std::str::FromStr;
use crate::base;
use crate::base::foundation;
use crate::base::kind::{DependencyKind, Kind, ProviderKind};
use crate::base::foundation::config;



pub trait DependencyConfig: foundation::config::DependencyConfig { }
pub trait ProviderConfig: foundation::config::ProviderConfig { }


/*

pub mod concrete {
    use std::str::FromStr;
    use crate::base;
    use crate::base::err::BaseErr;
    use crate::base::foundation::implementation::docker_daemon_foundation::postgres::PostgresDependencyConfig;
    use crate::space::parse::{CamelCase, DbCase};

    fn default_schema() -> DbCase {
        DbCase::from_str("PUBLIC").unwrap()
    }

    fn default_registry_database() -> DbCase {
        DbCase::from_str("REGISTRY").unwrap()
    }

    fn default_registry_provider_kind() -> CamelCase {
        CamelCase::from_str("Registry").unwrap()
    }
    impl PostgresDependencyConfig {
        pub fn create(config: Map) -> Result<Self, BaseErr> {
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

    impl base::config::DependencyConfig for PostgresDependencyConfig {


        fn kind(&self) -> &DependencyKind {
            todo!()
        }

        fn require(&self) -> Vec<Kind> {
            todo!()
        }
    }

    impl config::DependencyConfig for PostgresDependencyConfig {

        fn volumes(&self) -> HashMap<String, String> {
            self.postgres.volumes()
        }

    }

    impl docker_daemon_foundation::DependencyConfig for PostgresDependencyConfig {
        fn image(&self) -> String {
            self.image.clone()
        }
    }


}

 */



/*
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostgresDependencyConfig {
    pub postgres: PostgresClusterCoreConfig,
    pub docker: ProviderKind,
    pub image: String,
}

 */


