pub mod config;
pub mod err;
mod variants;

use std::sync::Arc;
use async_trait::async_trait;
use serde_derive::{Deserialize, Serialize};
use strum_macros::EnumDiscriminants;
use starlane_space::progress::Progress;
use starlane_space::status::{StatusDetail, StatusWatcher};
use crate::provider::err::ProviderErr;

#[derive(Clone, Debug, EnumDiscriminants, Serialize, Deserialize)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(ProviderKind))]
#[strum_discriminants(derive(Hash, Serialize, Deserialize))]
pub enum ProviderKindDef {
 ///
 Service
}


/// indicates which architecture layer manages this dependency or if management is external
/// to starlane itself.  Managing entails: downloading, installing and starting the [StatusEntity]
#[derive(Clone,Debug,Serialize, Deserialize)]
pub enum Manager {
    /// the [StatusEntity] is managed by Starlane's Foundation.  For example a running Starlane
    /// local development cluster might use DockerDesktopFoundation to provide services like
    /// Postgres.
    Foundation,

    /// The [StatusEntity] is managed by the Platform. Again a postgres example: instead of
    /// being responsible for starting Postgres imagine that the [StatusEntity] is actually
    /// a `PostgresConnectionPool` ... The Platform is responsible for creating and maintaining
    /// and status reporting on the health of the connection pool (which isn't the same as
    /// Foundation which is responsible for making the service available and in a ready state)
    Platform,

    /// A completely  external entity manages this [StatusEntity]
    External
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

    fn status(&self) -> StatusDetail;

    fn status_watcher(&self) -> Arc<tokio::sync::watch::Receiver<StatusDetail>>;

    /// [Provider::synchronize] triggers a state query and updates
    /// the [StatusDetail] as best it can be described
    async fn synchronize(&self);


    ///
    async fn provision(&self) -> Result<StatusWatcher,ProviderErr>;

}




