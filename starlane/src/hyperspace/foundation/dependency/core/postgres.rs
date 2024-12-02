use std::collections::HashMap;
use std::str::FromStr;
use derive_name::Name;
use futures::TryFutureExt;
use serde::{Deserialize, Serialize};
use crate::hyperspace::foundation;
use crate::hyperspace::foundation::config;
use crate::hyperspace::foundation::err::FoundationErr;
use crate::hyperspace::foundation::kind::{DependencyKind, Kind, PostgresKind, ProviderKind};
use crate::hyperspace::foundation::Dependency;
use crate::hyperspace::foundation::implementation::docker_desktop_foundation;
use crate::hyperspace::foundation::implementation::docker_desktop_foundation::{DependencyConfig};
use crate::hyperspace::foundation::util::Map;
use crate::space::parse::CamelCase;

#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct PostgresClusterCoreConfig {
    pub port: u16,
    pub data_dir: String,
    pub username: String,
    pub password: String,
    pub providers: HashMap<CamelCase, ProviderConfig>,
}


impl PostgresClusterCoreConfig {
    pub fn create(config: Map) -> Result<Self, FoundationErr> {
        let port: u16 = config.from_field_opt("port").map_err(FoundationErr::config_err)?.map_or(5432u16, |port| port);
        let username: String = config.from_field_opt("username").map_err(FoundationErr::config_err)?.map_or("postgres".to_string(), |username| username);
        let password : String = config.from_field_opt("password").map_err(FoundationErr::config_err)?.map_or("postgres".to_string(), |password| password);
        let data_dir: String = config.from_field("data_dir")?;

        let mut providers =  config.parse_same("providers"  )?;
        let registry_kind = CamelCase::from_str("Registry")?;
        if !providers.contains_key(&registry_kind) {
            providers.insert(registry_kind,ProviderConfig::default_registry());
        }

        Ok(PostgresClusterCoreConfig {
            port,
            data_dir,
            username,
            password,
            providers,
        })
    }
}



impl config::DependencyConfig for PostgresClusterCoreConfig {

    fn kind(&self) -> &DependencyKind {
        & DependencyKind::PostgresCluster
    }

    fn volumes(&self) -> HashMap<String, String> {
        let mut map = HashMap::new();
        map.insert("data".to_string(), self.data_dir.clone() );
        map
    }


    fn require(&self) -> &Vec<Kind> {
        &foundation::default_requirements()
    }


    fn clone_me(&self) -> Box<dyn config::DependencyConfig> {
        Box::new(self.clone())
    }
}

impl docker_desktop_foundation::DependencyConfig for PostgresClusterCoreConfig {

}




impl config::ProviderConfigSrc<ProviderConfig> for PostgresClusterCoreConfig {
    fn providers(&self) -> Result<&HashMap<CamelCase, ProviderConfig>, FoundationErr> {
        todo!()
    }

    fn provider(&self, kind: &CamelCase) -> Result<Option<&ProviderConfig>, FoundationErr> {
        todo!()
    }
}


#[derive(Name,Clone,Debug,Eq,PartialEq,Hash,strum_macros::Display,strum_macros::EnumString, strum_macros::IntoStaticStr,Serialize, Deserialize)]
pub enum PostgresSeed {
    Registry
}





#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct ProviderConfig {
    pub kind: PostgresKind,
    pub database: Option<String>,
    pub seed: Option<PostgresSeed>
}

impl ProviderConfig {
    pub fn default_registry() -> Self {
        Self {
            kind: PostgresKind::Registry,
            database: Some("/var/lib/postgresql/data".to_string()),
            seed: Some(PostgresSeed::Registry),
        }
    }
}

impl config::ProviderConfig for ProviderConfig {
    fn kind(&self) -> &ProviderKind {
        todo!()
    }

    fn clone_me(&self) -> Box<dyn config::ProviderConfig> {
        Box::new(self.clone())
    }
}

