/// # FOUNDATION
/// A ['Foundation'] provides abstracted control over the services and dependencies that drive Starlane.
/// Presently there is only the ['DockerDaemonFoundation'] which uses a local Docker Service
/// to pull dependent Docker Images, run docker instances and in general enables the Starlane [`Platform`]
/// manage the lifecycle of arbitrary services.
///
/// There are two sub concepts that ['Foundation'] provides: ['Dependency'] and  ['Provider'].
/// The [`FoundationConfig`] enumerates dependencies which are typically things that don't ship
/// with the Starlane binary.  Common examples are: Postgres, Keycloak, Docker.  Each config
/// implementation must know how to ready that Dependency and potentially even launch an
/// instance of that Dependency.  For Example: Postgres Database is a common implementation especially
/// because the default Starlane [`Registry`] (and at the time of this writing the only Registry support).
/// The Postgres [`Dependency`] ensures that Postgres is accessible and properly configured for the
/// Starlane Platform.
///
///
/// ## PROVIDER
/// A [`Dependency`] has a one to many child concept called a [`Provider`] (poorly named!)  Not all Dependencies
/// have a Provider.  A Provider is something of an instance of a given Dependency.... For example:
/// The Postgres Cluster [`DependencyKind::PostgresCluster`]  (talking the actual postgresql software which can serve multiple databases)
/// The Postgres Dependency may have multiple Databases ([`ProviderKind::Database`]).  The provider
/// utilizes a common Dependency to provide a specific service etc.
///
/// ## THE REGISTRY
/// There is one special implementation that the Foundation must manage which is the [`Foundation::registry`]
/// the Starlane Registry is the only required implementation from the vanilla Starlane installation




use crate::hyperspace::foundation::kind::{DependencyKind, FoundationKind, IKind, ProviderKind};
use crate::hyperspace::platform::PlatformConfig;
use futures::TryFutureExt;
use itertools::Itertools;
use serde;
use serde::de::{MapAccess, Visitor};
use serde::{de, Deserialize, Deserializer, Serialize};
use serde_yaml::Value;
use std::fmt::{Debug, Display};
use std::future::Future;
use std::hash::Hash;
use std::ops::{Deref, DerefMut};
use std::str::FromStr;
use crate::hyperspace::foundation::config::{Config, DependencyConfig, FoundationConfig, ProviderConfig};
use crate::hyperspace::foundation::err::FoundationErr;
use crate::hyperspace::foundation::state::State;
use crate::hyperspace::reg::Registry;
use crate::space::log::{Progress, Status};
use crate::space::parse::CamelCase;
use crate::space::progress::Progress;

pub mod factory;
pub mod runner;


//pub mod docker;
pub mod err;
pub mod kind;


pub mod config;


pub mod implementation;

pub mod util;


pub mod dependency;
pub mod state;

/// ['Foundation'] is an abstraction for managing infrastructure.
#[async_trait]
pub trait Foundation: Send + Sync
{
    fn kind(&self) -> &FoundationKind;

    fn config(&self) -> &impl FoundationConfig;


    /// Install and initialize any Dependencies and/or [`Providers`] that
    /// are required for this Foundation to run (usually this is not much more than whatever
    /// software is required to run the Registry.)
    fn install(&self, progress: Progress) -> Result<(), FoundationErr>;


    /// return the given [`Dependency`] if it exists within this [`Foundation`]
    fn dependency(&self, kind: &DependencyKind) -> Result<Option<impl Dependency>, FoundationErr>;

    /// return a handle to the [`Registry`]
    fn registry(&self) -> Result<Registry,FoundationErr>;
}

/// A [`Dependency`] is an add-on to the [`Foundation`] infrastructure which may need to be
/// downloaded, installed, initialized and started.
///
/// The Dependency facilitates instances via ['Provider'].  In other words if the Dependency
/// is a Database server like Postgres... the Dependency will download, install, initialize and
/// start the service whereas a Provider in this example would represent an individual Database
#[async_trait]
pub trait Dependency: Send + Sync
{
    fn kind(&self) -> &DependencyKind;

    fn config(&self) -> &impl DependencyConfig;

    fn state(&self) -> &State;

    /// perform any downloads for the Dependency
    async fn download(&self, progress: Progress) -> Result<(), FoundationErr>;

    /// install the dependency
    async fn install(&self, progress: Progress) -> Result<(), FoundationErr>;

    /// perform any steps needed to initialize the dependency
    async fn initialize(&self, progress: Progress) -> Result<(), FoundationErr>;


    /// Start the dependency (if appropriate)
    /// returns a LiveService which will keep the service alive until
    /// LiveService handle gets dropped
    async fn start(&self, progress: Progress) -> Result<LiveService<DependencyKind>, FoundationErr>;


    /// return a [`Provider`] which can create instances from this [`Dependency`]
    fn provider(&self, kind: &ProviderKind) -> Result<Option<impl Provider>, FoundationErr>;

}

/// A [`Provider`] is an 'instance' of this dependency... For example a Postgres Dependency
/// Installs
pub trait Provider: Sized {

    fn kind(&self) -> &ProviderKind;

    fn config(&self) -> &impl ProviderConfig;

    fn state(&self) -> &State;


    async fn initialize(&self, progress: Progress) -> Result<(), FoundationErr>;


    async fn start(&self, progress: Progress) -> Result<LiveService<CamelCase>, FoundationErr>;


}




#[derive(Clone, Serialize, Deserialize)]
pub struct StarlaneConfig {
    pub context: String,
    pub home: String,
    pub can_nuke: bool,
    pub can_scorch: bool,
    pub control_port: u16,
    //    pub foundation: ProtoFoundationSettings,
}


fn deserialize_from_value<'de, D>(deserializer: D) -> Result<Value, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Deserialize::deserialize(deserializer)?;
    serde_yaml::from_value(value).map_err(de::Error::custom)
}

#[derive(Clone)]
pub struct LiveService<K>
{
    name: String,
    kind: K,
    tx: tokio::sync::mpsc::Sender<()>,
}

impl <K> LiveService<K>
{
    pub fn new(name: String, kind: K, tx: tokio::sync::mpsc::Sender<()>) -> Self {
        Self { name, kind, tx }
    }
}




#[cfg(test)]
pub mod test {
    use crate::hyperspace::foundation::err::FoundationErr;

    #[test]
    pub fn test_builder() {
        fn inner() -> Result<(), FoundationErr> {
            let foundation = include_str!("../../../../config/foundation/docker-daemon.yaml");

            let foundation = foundation_config(foundation)?;


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