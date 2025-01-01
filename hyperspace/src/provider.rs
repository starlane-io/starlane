pub mod config;
pub mod err;
mod variants;

use std::sync::Arc;
use async_trait::async_trait;
use serde_derive::{Deserialize, Serialize};
use strum_macros::EnumDiscriminants;
use starlane_space::progress::Progress;
use starlane_space::status::{StatusDetail, StatusEntity, StatusWatcher};
use crate::provider::err::ProviderErr;

use starlane_space::status::Status;
use starlane_space::status::Stage;
use starlane_space::status::StateDetail;
use starlane_space::status::PendingDetail;

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
pub trait Provider: StatusEntity + Sync {
    type Config: config::ProviderConfig + ?Sized;

    type Item;

    fn kind(&self) -> ProviderKindDef;

    fn config(&self) -> Arc<Self::Config>;

    /// [Provider::probe] query the state of the concrete resource that this [Provider]
    /// is modeling and make this [Provider] model match the current Provision state.
    /// [Provider::probe] is especially useful when it comes to updating [StatusEntity::status]
    /// from [Status::unknown]
    async fn probe(&self) -> Result<(),ProviderErr>;


    /// Returns an interface clone for [Provider::Item] when it reaches [Status::Ready].
    /// If [Provider::Item] is not [Status::Ready] [Provider::ready] will start executing
    /// the necessary task and steps in order to produce a readied [Provider::Item]
    ///
    /// The [Provider::ready] should be reentrant--meaning it can be called multiple times without
    /// causing an error. A [Provider::ready] implementation should always first call
    /// [Provider::probe] to determine the last completed successful [Stage] and continue its
    /// remaining stages if possible.
    ///
    /// Calling [Provider::ready] on a [Provider] that's current [StateDetail]'s variant is
    /// [StateDetail::Pending] should `un-panic` the [Provider] and cause it to retry readying
    /// [Provider::Item]. A [Provider::ready] invocation on a [Provider] that is
    /// [StateDetail::Fatal] should fail immediately.
    ///
    /// Progress [Status] of [Self::ready] can be tracked using: [Self::status_watcher]
    async fn ready(&self) -> Result<Self::Item,ProviderErr>;

}




