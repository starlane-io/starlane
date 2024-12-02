use std::collections::HashMap;
use std::str::FromStr;
use serde::{Deserialize, Serialize};
use crate::hyperspace::foundation::config::{DependencyConfig, ProviderConfig};
use crate::hyperspace::foundation::kind::{DependencyKind, Kind, ProviderKind};
use crate::hyperspace::foundation::{LiveService, Provider};
use crate::space::parse::{CamelCase, DbCase, VarCase};
use crate::hyperspace::foundation::implementation::docker_desktop_foundation::Foundation;

fn default_schema() -> DbCase{
    DbCase::from_str("PUBLIC").unwrap()
}

fn default_registry_database() -> DbCase{
    DbCase::from_str("REGISTRY").unwrap()
}

fn default_registry_provider_kind() -> CamelCase{
    CamelCase::from_str("Registry").unwrap()
}



#[derive(Debug,Clone,Eq,PartialEq,Serialize,Deserialize)]
pub struct PostgresDependencyConfig {

    volumes: HashMap<String, String>,

    #[serde(default="Foundation::default_requirements")]
    require: Vec<Kind>,

    providers: HashMap<CamelCase,Box<dyn ProviderConfig>>
}

impl DependencyConfig for PostgresDependencyConfig{
    fn kind(&self) -> &DependencyKind {
        &DependencyKind::PostgresCluster
    }

    fn volumes(&self) -> &HashMap<String,String> {
        &self.volumes
    }

    fn require(&self) -> &Vec<Kind> {
        &self.require
    }


    fn providers(&self) -> &HashMap<CamelCase, Box<dyn ProviderConfig>> {
        &self.providers
    }

    fn provider(&self, kind: &ProviderKind) -> Option<Box<dyn ProviderConfig>> {
        self.providers.get(&kind.provider).cloned()
    }
}

#[derive(Debug,Clone,Eq,PartialEq,Serialize,Deserialize)]
pub struct PostgresProviderConfig {

    kind: CamelCase,

    #[serde(default="default_schema")]
    schema: DbCase,

    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    seed: Option<String>,
}


impl ProviderConfig for PostgresProviderConfig{
    fn kind(&self) -> &ProviderKind {
        &ProviderKind::new(DependencyKind::PostgresCluster,self.kind.clone())
    }
}



#[derive(Debug,Clone,Eq,PartialEq,Serialize,Deserialize)]
pub struct RegistryProviderConfig {

    #[serde(default="default_registry_provider_kind")]
    kind: CamelCase,
    #[serde(default="default_registry_database")]
    database: DbCase,
    #[serde(default="default_schema")]
    schema: DbCase,
}

impl RegistryProviderConfig {
    pub fn new( database: DbCase ) -> RegistryProviderConfig {
        Self {
            database,
            ..Default::default()
        }
    }
}

impl Default for RegistryProviderConfig {
    fn default() -> Self {
        let database = default_registry_database();
        let schema= default_schema();
        let kind = default_registry_provider_kind();

        Self {
            kind,
            database,
            schema,
        }
    }
}

impl ProviderConfig for RegistryProviderConfig {
    fn kind(&self) -> &ProviderKind {
        &ProviderKind::new(DependencyKind::PostgresCluster,self.kind.clone())
    }
}