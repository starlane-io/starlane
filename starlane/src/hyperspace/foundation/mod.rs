pub mod docker;

use std::collections::HashMap;
use crate::hyperspace::database::{Database, LiveDatabase};
use crate::hyperspace::platform::PlatformConfig;
use crate::hyperspace::registry::err::FoundationErr;
use crate::hyperspace::registry::postgres::embed::PostgresClusterConfig;
use async_trait::async_trait;
use port_check::is_local_ipv4_port_free;
use postgresql_embedded::{PostgreSQL, Settings};
use thiserror::Error;
use crate::hyperspace::registry::postgres::PostgresConnectInfo;
use crate::hyperspace::shutdown::{add_shutdown_hook, panic_shutdown};
use crate::space::parse::VarCase;

#[async_trait]
pub trait Foundation: Send + Sync + Sized
where
    Self: Sized,
    Self: 'static,
    Self::RegistryFoundation: RegistryFoundation,
{

    type RegistryFoundation;

    /// install any 3rd party dependencies this foundation requires to be minimally operable
    async fn install_foundation_required_dependencies(&self) -> Result<(), FoundationErr>;

    /// install a named dependency.  For example the dependency might be "Postgres." The implementing Foundation must
    /// be capable of installing that dependency.  The foundation will make the dependency available after installation
    /// although the method of installing the dependency is under the complete control of the Foundation.  For example:
    /// a LocalDevelopmentFoundation might have an embedded Postgres Database and perhaps another foundation: DockerDesktopFoundation
    /// may actually launch a Postgres Docker image and maybe a KubernetesFoundation may actually install a Postgres Operator ...
    async fn install_dependency(&self, key: &DependencyKey, args: Vec<String> ) -> Result<impl Dependency, FoundationErr>;

    /// return the RegistryFoundation
    fn registry(&self) -> &Self::RegistryFoundation;
}

pub struct FoundationConfig {
    pub registry: RegistryConfig2
}

pub struct RegistryConfig2 {

}

#[async_trait]
pub trait RegistryFoundation: DependencyFoundation<Config=Self::RegistryConfig> where
{

    type RegistryConfig;

    fn dependencies( &self ) -> &Vec<impl Dependency>;
}



impl DependencyFoundation for PostgresRegistryFoundation {
    type Config = PostgresRegistryFoundation::RegistryConfig;

    fn name() -> String {
        "PostgresRegistry".to_string()
    }

    fn dependency(&self) -> &impl Dependency<Err=FoundationErr> {

    }

    async fn install(&self, config: &Self::Config) -> Result<(), FoundationErr> {
        todo!()
    }

    async fn initialize(&self) -> Result<(), FoundationErr> {
        todo!()
    }

    async fn start(&self) -> Result<LiveDatabase, FoundationErr> {
        todo!()
    }
}



pub trait DependencyFoundation: Send + Sync + Sized
where
    FoundationErr: std::error::Error + Send + Sync,
    Self: Sized,
    Self: 'static,
    Self::Config: Sized + Send + Sync + 'static,
{
    type Config;

    fn name()  -> String;

    fn dependency(&self) -> & impl Dependency;

    /// install the dependency in the foundation.  This may be a third party
    /// software that Starlane relies upon (such as postgres for the registry)
    async fn install(& mut self, config: &Self::Config) -> Result<(), FoundationErr>;

    /// expects that `Self::install()` has installed 3rd party
    /// dependencies successfully.  `Self::initialize()` performs
    /// any initial setup that needs to occur before the dependencies can be used
    async fn initialize(& mut self) -> Result<(), FoundationErr>;

    /// Start the Dependency.
    async fn start(& mut self) -> Result<LiveDatabase, FoundationErr>;
}




impl RegistryFoundation for PostgresRegistryFoundation {
    type RegistryConfig = Database<PostgresClusterConfig>;

}

impl DependencyFoundation for PostgresRegistryFoundation {
    type Config = PostgresRegistryFoundation::RegistryConfig;

    fn name() -> String {
        todo!()
    }

    fn dependency(&self) -> & impl Dependency {
        todo!()
    }

    async fn install(&mut self, config: &Self::Config) -> Result<(), FoundationErr> {
        todo!()
    }

    async fn initialize(&mut self) -> Result<(), FoundationErr> {
        todo!()
    }

    async fn start(&mut self) -> Result<LiveDatabase, FoundationErr> {
        todo!()
    }
}

pub trait Dependency {

    fn key() -> DependencyKey;

    fn create( args: HashMap<VarCase,String>) -> Result<impl Dependency,FoundationErr>;

    async fn install(&mut self) -> Result<(), FoundationErr>;

    async fn start(&mut self) -> Result<LiveDatabase, FoundationErr>;

    async fn provider(&mut self, key: &ProviderKey, args: &HashMap<VarCase,String> ) -> Result<impl Provider,FoundationErr>;

}

pub trait Provider {
    async fn initialize(&mut self) -> Result<(), FoundationErr>;
}


#[derive(Clone)]
pub struct PostgresRegistryFoundation {
    pub config: PostgresClusterConfig,
}

impl RegistryFoundation for PostgresRegistryFoundation {
    type RegistryConfig = Database<PostgresClusterConfig>;

    fn dependencies(&self) -> &Vec<impl Dependency> {
        todo!()
    }
}




#[derive(Clone,Eq,PartialEq,Hash,strum_macros::Display)]
pub enum DependencyKey {
    Postgres,
    #[strum(to_string = "{0}")]
    Ext(String)
}

#[derive(Clone,Eq,PartialEq,Hash)]
pub struct ProviderKey{
    dep: DependencyKey,
    kind: ProviderKind
}

impl ToString for ProviderKey {
    fn to_string(&self) -> String {
        format!("<{}:{}>", self.dep, self.kind)
    }
}

#[derive(Clone,Eq,PartialEq,Hash,strum_macros::Display)]
pub enum ProviderKind {
    Any,
    #[strum(to_string = "{0}")]
    Ext(String)
}


#[derive(Clone)]
pub struct LiveService<S> where S: Clone{
    pub service: S,
    tx: tokio::sync::mpsc::Sender<()>
}


#[derive(Error)]
pub enum FoundationErr {
    #[error("[{key}] Error: '{msg}'")]
    DepErr{ key: DependencyKey, msg: String},
    #[error("[{key}] Error: '{msg}'")]
    ProviderErr{ key: ProviderKey, msg: String},
    #[error("error converting foundation args for dependency: '{key}' serialization err: '{err}' from args: {args}")]
    DepConfErr { key: DependencyKey,err: serde_json::Error, args: HashMap<VarCase, String>},
    #[error("error converting foundation args for provider: '{key}' serialization err: '{err}' from args: {args}")]
    ProvConfErr { key: ProviderKey, err: serde_json::Error, args: HashMap<VarCase, String>},
}

impl FoundationErr {
    pub fn dep_conf_err( key: DependencyKey, err: serde_json::Error, args: HashMap<VarCase, String>) -> Self {
        Self::DepConfErr {key,err,args}
    }

    pub fn prov_conf_err( key: ProviderKey, err: serde_json::Error, args: HashMap<VarCase, String>) -> Self {
        Self::ProvConfErr {key,err,args}
    }
}
