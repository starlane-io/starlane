use std::collections::HashMap;
use derive_name::Name;
use futures::TryFutureExt;
use serde::{Deserialize, Serialize};
use crate::hyperspace::foundation::config;
use crate::hyperspace::foundation::config::ProviderConfig;
use crate::hyperspace::foundation::err::FoundationErr;
use crate::hyperspace::foundation::kind::{DependencyKind, PostgresKind, ProviderKind};
use crate::hyperspace::foundation::Dependency;
use crate::hyperspace::foundation::util::Map;
use crate::space::parse::CamelCase;

#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct PostgresClusterConfig {
    pub port: u16,
    pub data_dir: String,
    pub username: String,
    pub password: String,
    pub providers: HashMap<CamelCase,PostgresProviderConfig>,
}


impl PostgresClusterConfig {
    pub fn create(config: Map) -> Result<impl config::DependencyConfig, FoundationErr> {
        let port: u16 = config.from_field_opt("port").map_err(FoundationErr::config_err)?.map_or(5432u16, |port| port);
        let username: String = config.from_field_opt("username").map_err(FoundationErr::config_err)?.map_or("postgres".to_string(), |username| username);
        let password : String = config.from_field_opt("password").map_err(FoundationErr::config_err)?.map_or("postgres".to_string(), |password| password);
        let data_dir: String = config.from_field("data_dir")?;

        let providers =  config.parse_same("providers"  )?;

        Ok(PostgresClusterConfig {
            port,
            data_dir,
            username,
            password,
            providers,
        })
    }
}

impl config::DependencyConfig for PostgresClusterConfig {

    fn kind(&self) -> &DependencyKind {
        & DependencyKind::PostgresCluster
    }

    fn volumes(&self) -> &Vec<String> {
        let volumes = vec![self.data_dir.clone()];
        &volumes
    }

    fn provider_kinds(&self) -> Vec<CamelCase> {
        self.providers.keys().clone().collect()
    }

    fn provider(&self, kind: &CamelCase) -> Option<&impl ProviderConfig> {
        self.providers.get(kind)
    }
}


#[derive(Name,Clone,Debug,Eq,PartialEq,Hash,strum_macros::Display,strum_macros::EnumString, strum_macros::IntoStaticStr,Serialize, Deserialize)]
pub enum PostgresSeed {
    Registry
}

#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct PostgresProviderConfig {
    pub kind: PostgresKind,
    pub database: Option<String>,
    pub seed: Option<PostgresSeed>
}

