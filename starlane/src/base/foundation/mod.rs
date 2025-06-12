use crate::base;
use crate::base::err::BaseErr;
/// # FOUNDATION
///
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
/// The Postgres Cluster [`crate::base::kind::DependencyKind::PostgresCluster`]  (talking the actual postgresql software which can serve multiple databases)
/// The Postgres Dependency may have multiple Databases ([`crate::base::kind::ProviderKind::Database`]).  The provider
/// utilizes a common Dependency to provide a specific service etc.
///
/// ## THE REGISTRY
/// There is one special core that the Foundation must manage which is the [`Foundation::registry`]
/// the Starlane Registry is the only required core from the vanilla Starlane installation
use crate::base::foundation::kind::FoundationKind;
use crate::base::foundation::status::{Phase, Status, StatusDetail};
use crate::base::foundation::util::CreateProxy;
/// # FOUNDATION
///
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
/// The Postgres Cluster [`crate::base::kind::DependencyKind::PostgresCluster`]  (talking the actual postgresql software which can serve multiple databases)
/// The Postgres Dependency may have multiple Databases ([`crate::base::kind::ProviderKind::Database`]).  The provider
/// utilizes a common Dependency to provide a specific service etc.
///
/// ## THE REGISTRY
/// There is one special core that the Foundation must manage which is the [`Foundation::registry`]
/// the Starlane Registry is the only required core from the vanilla Starlane installation
use crate::base::kind::{DependencyKind, IKind, Kind, ProviderKind};
use crate::hyperspace::platform::PlatformConfig;
use crate::space::parse::CamelCase;
use crate::space::progress::Progress;
use base::registry;
use downcast_rs::{impl_downcast, Downcast, DowncastSync};
use futures::TryFutureExt;
use itertools::Itertools;
use once_cell::sync::Lazy;
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
use tokio::sync::watch::Receiver;
use crate::env::config;
use crate::hyperspace::err::HypErr;
//pub mod proxy;

//pub mod runner;

/// [`skel`] provides a starter implementation of [`foundation`] where all the traits are extended
pub mod skel;

//pub mod docker;
pub mod kind;

pub mod config;

pub mod implementation;

pub mod util;

pub mod status;

static REQUIRED: Lazy<Vec<Kind>> = Lazy::new(|| vec![]);

pub fn default_requirements() -> Vec<Kind> {
    REQUIRED.clone()
}

pub trait Types {
    type Foundation;

    type Dependency;

    type Provider;
}


/// ['Foundation'] is an abstraction for managing infrastructure.
#[async_trait]
pub trait Foundation: Downcast + Sync + Send {
    /// [`Foundation::Config`] should be a `concrete` implementation of [`base::config::FoundationConfig`]
    type Config: base::config::FoundationConfig + ?Sized;

    /// [`Foundation::Dependency`] Should be [`Dependency`] or a custom `trait` that implements [`Dependency`] ... it should not be a concrete implementation
    type Dependency: Dependency + ?Sized;

    /// [`Foundation::Provider`] Should be [`Provider`] or a custom `trait` that implements [`Provider`] ... it should not be a concrete implementation
    type Provider: Provider + ?Sized;

    fn kind(&self) -> FoundationKind;

    fn config(&self) -> Arc<Self::Config>;

    fn status(&self) -> Status;


    async fn status_detail(&self) -> Result<StatusDetail, BaseErr>;

    fn status_watcher(&self) -> Arc<tokio::sync::watch::Receiver<Status>>;

    /// synchronize must be called first.  In this method the [`Foundation`] will check
    /// update the present [Foundation::status] to be consistent with the actual infrastructure
    async fn synchronize(&self, progress: Progress) -> Result<Status, BaseErr>;

    /// Install and initialize any Dependencies and/or [`Providers`] that
    /// are required for this Foundation to run (usually this is not much more than whatever
    /// software is required to run the Registry.)
    async fn install(&self, progress: Progress) -> Result<(), BaseErr>;

    /// return the given [`Dependency`] if it exists within this [`Foundation`]
    fn dependency(&self, kind: &DependencyKind) -> Result<Option<Box<Self::Dependency>>, BaseErr>;

    /// return a handle to the [`Registry`]
    fn registry(&self) -> Result<registry::Registry, BaseErr>;
}

impl_downcast!(Foundation assoc Config, Dependency, Provider);

/// A [`Dependency`] is an add-on to the [`Foundation`] infrastructure which may need to be
/// downloaded, installed, initialized and started.
///
/// The Dependency facilitates instances via ['Provider'].  In other words if the Dependency
/// is a Database server like Postgres... the Dependency will download, install, initialize and
/// start the service whereas a Provider in this example would represent an individual Database
#[async_trait]
pub trait Dependency: Downcast + Send + Sync {
    type Config: base::config::DependencyConfig + ?Sized;

    type Provider: Provider + ?Sized;

    fn kind(&self) -> DependencyKind;

    fn config(&self) -> Arc<Self::Config>;

    fn status(&self) -> Status;

    fn status_watcher(&self) -> Arc<tokio::sync::watch::Receiver<Status>>;

    /// perform any downloads for the Dependency
    async fn download(&self, progress: Progress) -> Result<(), BaseErr>;

    /// install the dependency
    async fn install(&self, progress: Progress) -> Result<(), BaseErr>;

    /// perform any steps needed to initialize the dependency
    async fn initialize(&self, progress: Progress) -> Result<(), BaseErr>;

    /// Start the dependency (if appropriate)
    /// returns a LiveService which will keep the service alive until
    /// LiveService handle gets dropped
    async fn start(&self, progress: Progress)
                   -> Result<LiveService<DependencyKind>, BaseErr>;

    /// return a [`Provider`] which can create instances from this [`Dependency`]
    fn provider(&self, kind: &ProviderKind) -> Result<Option<Box<Self::Provider>>, BaseErr>;
}

impl_downcast!(Dependency assoc Config, Provider);

/// A [`Provider`] is an 'instance' of this dependency... For example a Postgres Dependency
/// Installs
#[async_trait]
pub trait Provider: Downcast + Send + Sync {
    type Config: base::config::ProviderConfig + ?Sized;

    fn kind(&self) -> &ProviderKind;

    fn config(&self) -> Arc<Self::Config>;

    fn status(&self) -> Status;

    fn status_watcher(&self) -> Arc<tokio::sync::watch::Receiver<Status>>;

    async fn initialize(&self, progress: Progress) -> Result<(), BaseErr>;

    async fn start(&self, progress: Progress) -> Result<LiveService<CamelCase>, BaseErr>;
}

impl_downcast!(Provider assoc Config);

#[derive(Clone, Serialize, Deserialize)]
pub struct StarlaneConfig {
    pub context: String,
    pub home: String,
    pub can_nuke: bool,
    pub can_scorch: bool,
    pub control_port: u16,
    //    pub foundation: ProtoFoundationSettings,
}

impl Default for StarlaneConfig {
    fn default() -> Self {
        if let Ok(Some(config)) = config() {
            config
        }
        else {
            todo!("StarlaneConfig::default() not yet implemented")
        }
    }
}

fn deserialize_from_value<'de, D>(deserializer: D) -> Result<Value, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Deserialize::deserialize(deserializer)?;
    serde_yaml::from_value(value).map_err(de::Error::custom)
}

#[derive(Clone)]
pub struct LiveService<K> {
    name: String,
    kind: K,
    tx: tokio::sync::mpsc::Sender<()>,
}

impl<K> LiveService<K> {
    pub fn new(name: String, kind: K, tx: tokio::sync::mpsc::Sender<()>) -> Self {
        Self { name, kind, tx }
    }
}

pub(crate) struct FoundationSafety<F>
where
    F: Foundation,
{
    foundation: Box<F>,
}

#[async_trait]
impl<F> Foundation for FoundationSafety<F>
where
    F: Foundation,
{
    type Config = F::Config;
    type Dependency = F::Dependency;

    type Provider = F::Provider;

    fn kind(&self) -> FoundationKind {
        self.foundation.kind()
    }

    fn config(&self) -> Arc<Self::Config> {
        self.foundation.config()
    }

    fn status(&self) -> Status {
        self.status()
    }

    async fn status_detail(&self) -> Result<StatusDetail, BaseErr> {
        todo!()
    }

    fn status_watcher(&self) -> Arc<Receiver<Status>> {
        self.foundation.status_watcher()
    }

    async fn synchronize(&self, progress: Progress) -> Result<Status, BaseErr> {
        self.foundation.synchronize(progress).await
    }

    async fn install(&self, progress: Progress) -> Result<(), BaseErr> {
        if self.status().phase == Phase::Unknown {
            Err(BaseErr::unknown_state("install"))
        } else {
            self.foundation.install(progress).await
        }
    }

    fn dependency(&self, kind: &DependencyKind) -> Result<Option<Box<Self::Dependency>>, BaseErr> {
        if self.status().phase == Phase::Unknown {
            Err(BaseErr::unknown_state("dependency"))
        } else {
            self.foundation.dependency(kind)
        }
    }

    fn registry(&self) -> Result<registry::Registry, BaseErr> {
        if self.status().phase == Phase::Unknown {
            Err(BaseErr::unknown_state("registry"))
        } else {
            self.foundation.registry()
        }
    }
}

impl<F> CreateProxy for FoundationSafety<F>
where
    F: Foundation,
{
    type Proxy = F;

    fn proxy(&self) -> Result<Self::Proxy, BaseErr> {
        todo!()
    }
}

pub mod default {
    use crate::base;
    pub type Provider = Box<dyn super::Provider<Config=base::config::default::ProviderConfig>>;
    pub type Dependency =
    Box<dyn super::Dependency<Config=base::config::default::DependencyConfig, Provider=Provider>>;
    pub type Foundation = Box<
        dyn super::Foundation<
            Config=base::config::default::FoundationConfig,
            Dependency=Dependency,
            Provider=Provider,
        >,
    >;
}

#[cfg(test)]
pub mod test {}
