/// # FOUNDATION
/// A ['Foundation'] provides abstracted control over the services and dependencies that drive Starlane.
/// Presently there is only the ['DockerDaemonFoundation'] which uses a local Docker Service
/// to pull dependent Docker Images, run docker instances and in general enables the Starlane [`Platform`]
/// manage the lifecycle of arbitrary services.
///
/// There are two sub concepts that ['Foundation'] provides: ['Dependency'] and  ['Provider'].
/// The [`FoundationConfig`] enumerates dependencies which are typically things that don't ship
/// with the Starlane binary.  Common examples are: Postgres, Keycloak, Docker.  Each config
/// core must know how to ready that Dependency and potentially even launch an
/// instance of that Dependency.  For Example: Postgres Database is a common core especially
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
/// There is one special core that the Foundation must manage which is the [`Foundation::registry`]
/// the Starlane Registry is the only required core from the vanilla Starlane installation




use crate::hyperspace::foundation::kind::{DependencyKind, FoundationKind, IKind, Kind, ProviderKind};
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
use std::sync::Arc;
use once_cell::sync::Lazy;
use tokio::sync::watch::Receiver;
use crate::hyperspace::foundation::config::{Config, DependencyConfig, FoundationConfig, ProviderConfig};
use crate::hyperspace::foundation::err::{ActionRequest, FoundationErr};
use crate::hyperspace::foundation::status::{Phase, Status, StatusDetail};
use crate::hyperspace::reg::Registry;
use crate::space::parse::CamelCase;
use crate::space::progress::Progress;

pub mod runner;


//pub mod docker;
pub mod err;
pub mod kind;


pub mod config;


pub mod implementation;

pub mod util;


pub mod dependency;
pub mod status;

static REQUIRED: Lazy<Vec<Kind>> = Lazy::new(|| {
    vec![]
});

/// ['Foundation'] is an abstraction for managing infrastructure.
#[async_trait]
pub trait Foundation
{
    fn kind(&self) -> &FoundationKind;

    fn config(&self) -> &Box<dyn FoundationConfig>;


    fn status(&self) -> Status;

    fn status_watcher(&self) -> Arc<tokio::sync::watch::Receiver<Status>>;

    /// synchronize must be called first.  In this method the [`Foundation`] will check its
    /// environment to determine the
    async fn synchronize(&self, progress: Progress) -> Result<Status,FoundationErr>;


    /// Install and initialize any Dependencies and/or [`Providers`] that
    /// are required for this Foundation to run (usually this is not much more than whatever
    /// software is required to run the Registry.)
    async fn install(&self, progress: Progress) -> Result<(), FoundationErr>;


    /// return the given [`Dependency`] if it exists within this [`Foundation`]
    fn dependency(&self, kind: &DependencyKind) -> Result<Option<Box<dyn Dependency>>, FoundationErr>;

    /// return a handle to the [`Registry`]
    fn registry(&self) -> Result<Registry,FoundationErr>;

    fn default_requirements() -> Vec<Kind> {
        REQUIRED.clone()
    }
}

/// A [`Dependency`] is an add-on to the [`Foundation`] infrastructure which may need to be
/// downloaded, installed, initialized and started.
///
/// The Dependency facilitates instances via ['Provider'].  In other words if the Dependency
/// is a Database server like Postgres... the Dependency will download, install, initialize and
/// start the service whereas a Provider in this example would represent an individual Database
#[async_trait]
pub trait Dependency
{
    fn kind(&self) -> &DependencyKind;

    fn config(&self) -> &Box<dyn DependencyConfig>;

    fn status(&self) -> Status;

    fn status_watcher(&self) -> Arc<tokio::sync::watch::Receiver<Status>>;

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
    fn provider(&self, kind: &ProviderKind) -> Result<Option<Box<dyn Provider>>, FoundationErr>;

}

/// A [`Provider`] is an 'instance' of this dependency... For example a Postgres Dependency
/// Installs
#[async_trait]
pub trait Provider {

    fn kind(&self) -> &ProviderKind;

    fn config(&self) -> &Box<dyn ProviderConfig>;

    fn status(&self) -> Status;

    fn status_watcher(&self) -> Arc<tokio::sync::watch::Receiver<Status>>;

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


pub(crate) struct FoundationSafety {
    foundation: dyn Foundation
}

#[async_trait]
impl Foundation for FoundationSafety  {
    fn kind(&self) -> &FoundationKind {
        self.foundation.kind()
    }

    fn config(&self) -> &Box<dyn FoundationConfig>{
        self.foundation.config()
    }


    fn status(&self) -> Status{
        self.foundation.status()
    }

    fn status_watcher(&self) -> Arc<Receiver<Status>> {
        self.foundation.status_watcher()
    }

    async fn synchronize(&self, progress: Progress) -> Result<Status, FoundationErr> {
        self.foundation.synchronize(progress).await
    }

    async fn install(&self, progress: Progress) -> Result<(), FoundationErr> {
        if self.status().phase == Phase::Unknown {
            Err(FoundationErr::unknown_state("install"))
        } else {
            self.foundation.install(progress).await
        }
    }

    fn dependency(&self, kind: &DependencyKind) -> Result<Option<Box<dyn Dependency>>, FoundationErr> {
        if self.status().phase == Phase::Unknown {
            Err(FoundationErr::unknown_state("dependency"))
        } else {
            self.foundation.dependency(kind)
        }
    }

    fn registry(&self) -> Result<Registry, FoundationErr> {
        if self.status().phase == Phase::Unknown {
            Err(FoundationErr::unknown_state("registry"))
        } else {
            self.foundation.registry()
        }
    }
}


#[cfg(test)]
pub mod test {}