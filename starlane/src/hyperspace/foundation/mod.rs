use crate::hyperspace::foundation::config::{DependencyConfig, FoundationConfig};
use crate::hyperspace::foundation::err::{ActionRequest, FoundationErr};
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
/// The Postgres Cluster [`DependencyKind::PostgresCluster`]  (talking the actual postgresql software which can serve multiple databases)
/// The Postgres Dependency may have multiple Databases ([`ProviderKind::Database`]).  The provider
/// utilizes a common Dependency to provide a specific service etc.
///
/// ## THE REGISTRY
/// There is one special core that the Foundation must manage which is the [`Foundation::registry`]
/// the Starlane Registry is the only required core from the vanilla Starlane installation
use crate::hyperspace::foundation::kind::{
    DependencyKind, FoundationKind, IKind, Kind, ProviderKind,
};
use crate::hyperspace::foundation::status::{Phase, Status, StatusDetail};
use crate::hyperspace::foundation::util::CreateProxy;
use crate::hyperspace::platform::PlatformConfig;
use crate::hyperspace::reg::Registry;
use crate::space::parse::CamelCase;
use crate::space::progress::Progress;
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
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::watch::Receiver;

pub mod proxy;

pub mod runner;

//pub mod docker;
pub mod err;
pub mod kind;

pub mod config;

pub mod implementation;

pub mod util;

pub mod dependency;
pub mod status;

static REQUIRED: Lazy<Vec<Kind>> = Lazy::new(|| vec![]);

pub fn default_requirements() -> Vec<Kind> {
    REQUIRED.clone()
}

/// ['Foundation'] is an abstraction for managing infrastructure.
#[async_trait]
pub trait Foundation: Downcast + Sync + Send {
    type Config: FoundationConfig + Clone;
    type Types: FoundationTypeTraits;

    type Dependency: Dependency;

    type Provider: Provider;

    fn kind(&self) -> FoundationKind;

    fn config(&self) -> Self::Config;

    fn status(&self) -> Status;

    fn status_watcher(&self) -> Arc<tokio::sync::watch::Receiver<Status>>;

    /// synchronize must be called first.  In this method the [`Foundation`] will check its
    /// environment to determine the
    async fn synchronize(&self, progress: Progress) -> Result<Status, FoundationErr>;

    /// Install and initialize any Dependencies and/or [`Providers`] that
    /// are required for this Foundation to run (usually this is not much more than whatever
    /// software is required to run the Registry.)
    async fn install(&self, progress: Progress) -> Result<(), FoundationErr>;

    /// return the given [`Dependency`] if it exists within this [`Foundation`]
    fn dependency(&self, kind: &DependencyKind) -> Result<Option<Self::Dependency>, FoundationErr>;

    /// return a handle to the [`Registry`]
    fn registry(&self) -> Result<Registry, FoundationErr>;
}

/// [`FoundationTypeTraits`] should reference the best trait fit for [`Foundation`] types.
/// whereas the implementing Foundation may map its types to concrete structs or enums--
/// [`FoundationTypeTraits`] should only ever constrain to a trait. [`FoundationTypeTraits`]
/// is used by [`runner::FoundationProxy`] in order to create workable proxies for [`Dependency`]
/// and [`Provider`].  Notice that [`Foundation::Config`] is NOT included... this is because
/// configs are static and the [`runner::FoundationProxy`] should always
pub trait FoundationTypeTraits {
    type Dependency: Dependency;

    type Provider: Provider;
}

#[derive(Default)]
pub struct FoundationTypes<F> where F: Foundation {
    phantom: PhantomData<F>
}

/*
impl <F> FoundationTypeTraits for FoundationTypes<F> where F: Foundation, Self::Dependency: F::Dependency{

}

 */




/// call [`EasyFoundationTypeTraits::default`]
/// ```
/// use starlane::hyperspace::foundation::{implementation, Dependency, EasyFoundationTypeTraits, Foundation, Provider};
/// use implementation::docker_daemon_foundation as foundation;
///
/// let easy : EasyFoundationTypeTraits<foundation::DockerDaemonFoundation,foundation::Dependency,foundation::Provider> = EasyFoundationTypeTraits::default();///
#[derive(Default)]
pub struct EasyFoundationTypeTraits<F, D, P>(PhantomData<F>, PhantomData<D>, PhantomData<P>)
where
    F: Foundation,
    D: Dependency<Config = F::Dependency::Config, Provider = P>,
    P: Provider<Config = F::Dependency::Config>;






//pub type FoundationTypeTraitsCheat<D:Dependency<Provider=P>,P> = dyn FoundationTypeTraits<Dependency=D, Provider=P>;

impl_downcast!(Foundation assoc Config);

/// A [`Dependency`] is an add-on to the [`Foundation`] infrastructure which may need to be
/// downloaded, installed, initialized and started.
///
/// The Dependency facilitates instances via ['Provider'].  In other words if the Dependency
/// is a Database server like Postgres... the Dependency will download, install, initialize and
/// start the service whereas a Provider in this example would represent an individual Database
#[async_trait]
pub trait Dependency: Downcast + Send + Sync {
    type Config: config::DependencyConfig + Clone;

    type Provider: Provider;

    fn kind(&self) -> DependencyKind;

    fn config(&self) -> Self::Config;

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
    async fn start(&self, progress: Progress)
        -> Result<LiveService<DependencyKind>, FoundationErr>;

    /// return a [`Provider`] which can create instances from this [`Dependency`]
    fn provider(&self, kind: &ProviderKind) -> Result<Option<Self::Provider>, FoundationErr>;
}

impl_downcast!(Dependency assoc Config);

/// A [`Provider`] is an 'instance' of this dependency... For example a Postgres Dependency
/// Installs
#[async_trait]
pub trait Provider: Downcast + Send + Sync {
    type Config: config::ProviderConfig + Clone;

    fn kind(&self) -> &ProviderKind;

    fn config(&self) -> Self::Config;

    fn status(&self) -> Status;

    fn status_watcher(&self) -> Arc<tokio::sync::watch::Receiver<Status>>;

    async fn initialize(&self, progress: Progress) -> Result<(), FoundationErr>;

    async fn start(&self, progress: Progress) -> Result<LiveService<CamelCase>, FoundationErr>;
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
    foundation: Box<dyn F>,
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

    fn config(&self) -> Self::Config {
        self.foundation.config()
    }

    fn status(&self) -> Status {
        self.status()
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

    fn dependency(&self, kind: &DependencyKind) -> Result<Option<Self::Dependency>, FoundationErr> {
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

impl<F> CreateProxy for FoundationSafety<F>
where
    F: Foundation,
{
    type Proxy = F;

    fn proxy(&self) -> Result<Self::Proxy, FoundationErr> {}
}

pub mod default {
    use crate::hyperspace::foundation::config;
    pub type Provider = Box<dyn super::Provider<Config = config::default::ProviderConfig>>;
    pub type Dependency =
        Box<dyn super::Dependency<Config = config::default::DependencyConfig, Provider = Provider>>;
    pub type Foundation = Box<
        dyn super::Foundation<
            Config = config::default::FoundationConfig,
            Dependency = Dependency,
            Provider = Provider,
        >,
    >;
}

#[cfg(test)]
pub mod test {}
