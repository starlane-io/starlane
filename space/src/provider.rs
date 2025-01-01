pub mod config;
pub mod status;
pub mod err;
mod variants;

use std::sync::Arc;
use async_trait::async_trait;
use serde_derive::{Deserialize, Serialize};
use strum_macros::EnumDiscriminants;
use crate::progress::Progress;
use crate::provider::status::Status;

#[derive(Clone, Debug, EnumDiscriminants, Serialize, Deserialize)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(ProviderKind))]
#[strum_discriminants(derive(Hash, Serialize, Deserialize))]
pub enum ProviderKindDef {
 ///
 Service
}

/// A [`Provider`] is an add-on to the [`Foundation`] infrastructure which may need to be
/// downloaded, installed, initialized and started.
///
/// The Dependency facilitates instances via ['Provider'].  In other words if the Dependency
/// is a Database server like Postgres... the Dependency will download, install, initialize and
/// start the service whereas a Provider in this example would represent an individual Database
#[async_trait]
pub trait Provider: Send + Sync {
    type Config: config::ProviderConfig + ?Sized;

    fn kind(&self) -> ProviderKindDef;

    fn config(&self) -> Arc<Self::Config>;

    fn status(&self) -> Status;

    fn status_watcher(&self) -> Arc<tokio::sync::watch::Receiver<Status>>;

    /// [Provider::synchronize] triggers a state query and updates
    /// the [Status] as best it can be described
    async fn synchronize(&self);

    /// perform any fetching operations for the Dependency
    async fn fetch(&self, progress: Progress) -> Result<(), BaseErr>;

    /// install the dependency
    async fn install(&self, progress: Progress) -> Result<(), BaseErr>;

    /// perform any steps needed to initialize the dependency
    async fn initialize(&self, progress: Progress) -> Result<(), BaseErr>;

    /// Start the dependency (if appropriate)
    /// returns a LiveService which will keep the service alive until
    /// LiveService handle gets dropped
    async fn start(&self, progress: Progress)
                   -> Result<LiveService<DependencyKind>, BaseErr>;

    /// return a [`Provider`] which can create instances from this [`Provider`]
    fn provider(&self, kind: &ProviderKindDef) -> Result<Option<Box<Self::Provider>>, BaseErr>;
}


