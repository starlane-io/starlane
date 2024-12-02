use std::collections::HashMap;
use std::ops::Deref;
use std::str::FromStr;
use serde::{Deserialize, Serialize};
use crate::hyperspace::foundation::kind::{DependencyKind, Kind, ProviderKind};
use crate::hyperspace::foundation::{config, LiveService, Provider};
use crate::hyperspace::foundation::dependency::core::postgres::PostgresClusterCoreConfig;
use crate::hyperspace::foundation::err::FoundationErr;
use crate::hyperspace::foundation::implementation::docker_desktop_foundation;
use crate::space::parse::{CamelCase, DbCase, VarCase};
use crate::hyperspace::foundation::implementation::docker_desktop_foundation::Foundation;
use crate::hyperspace::foundation::util::Map;

fn default_schema() -> DbCase{
    DbCase::from_str("PUBLIC").unwrap()
}

fn default_registry_database() -> DbCase{
    DbCase::from_str("REGISTRY").unwrap()
}

fn default_registry_provider_kind() -> CamelCase{
    CamelCase::from_str("Registry").unwrap()
}



#[derive(Debug,Clone,Serialize,Deserialize)]
pub struct PostgresDependencyConfig {
    pub core: PostgresClusterCoreConfig,
    pub image: String
}

impl PostgresDependencyConfig {
    pub fn create( config: Map ) -> Result<Self,FoundationErr> {
        let core = PostgresClusterCoreConfig::create(config.clone())?;
        let image = config.from_field("image")?;
        Ok( PostgresDependencyConfig{core,image} )
    }
}

impl Deref for PostgresDependencyConfig {
    type Target = PostgresClusterCoreConfig;

    fn deref(&self) -> &Self::Target {
        &self.core
    }
}

impl config::DependencyConfig for PostgresDependencyConfig {
    fn kind(&self) -> &DependencyKind {
        todo!()
    }

    fn volumes(&self) -> HashMap<String, String> {
        todo!()
    }

    fn require(&self) -> &Vec<Kind> {
        todo!()
    }

    fn clone_me(&self) -> Box<dyn config::DependencyConfig> {
        Box::new(self.clone())
    }
}

impl docker_desktop_foundation::DependencyConfig for PostgresDependencyConfig{
    fn image(&self) -> &String {
        &self.image
    }
}


