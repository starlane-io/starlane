pub mod config;
pub mod context;
mod detail;
pub mod err;

use std::hash::{Hash, Hasher};
use async_trait::async_trait;
use serde_derive::{Deserialize, Serialize};
use starlane_space::parse::CamelCase;
use starlane_space::status::{
    Action, ActionRequest, Entity, EntityReadier, PendingDetail, StatusProbe,
};
use std::sync::Arc;
use strum_macros::EnumDiscriminants;

use crate::registry::Registry;
use starlane_space::status::Status;
use crate::base::{kinds, BaseSub};
use crate::base::config::BaseConfig;
use crate::base::kinds::Kind;

#[derive(Clone, Debug, EnumDiscriminants, Serialize, Deserialize)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(ProviderKind))]
#[strum_discriminants(derive(Hash, Serialize, Deserialize, strum_macros::Display))]
pub enum ProviderKindDef {
    /// [Provider::probe] should ascertain if the docker daemon is installed and running.
    /// If the DockerDaemon is accessible set [Status::Ready].
    /// If not accessible set [Status::Pending] with an [ActionRequest] providing helpful guidance
    /// to the Starlane admin on how to rectify the issue.
    ///
    /// Note: that the DockerDaemon [Provider] should take any steps to install or start Docker
    /// Daemon because Starlane is not keen on installing raw binaries for purposes of security...
    /// The whole point of the DockerDaemon dependency is to provide a way to extend Starlane using
    /// secure containers
    DockerDaemon,
    /// Represents a postgres cluster instance that serves [ProviderKindDef::PostgresDatabase]
    PostgresService,
    /// depends upon a readied [ProviderKindDef::PostgresService]
    PostgresDatabase(PostgresDatabaseKind),
    /// depends upon [ProviderKindDef::PostgresDatabase]::[PostgresDatabaseKindDef::Registry]
    Registry,
    /// [ProviderKindDef::_Ext] defines a new [ProviderKind] that is not builtin to Starlane
    _Ext(CamelCase),
}

/*
impl kinds::ProviderKind for ProviderKind{ }

impl Eq for ProviderKindDef {}

impl PartialEq<Self> for ProviderKindDef {
    fn eq(&self, other: &Self) -> bool {
        todo!()
    }
}

impl Hash for ProviderKindDef {
    fn hash<H: Hasher>(&self, state: &mut H) {
        todo!()
    }
}

 */


#[derive(Clone, Debug, EnumDiscriminants, Serialize, Deserialize)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(PostgresDatabaseKind))]
#[strum_discriminants(derive(Hash, Serialize, Deserialize))]
pub enum PostgresDatabaseKindDef {
    /// just a plain, empty postgres database full of potential
    Default,
    /// a variant of [ProviderKindDef::PostgresDatabase] that is initialized with the [Registry]
    /// sql schema to be utilized by a
    Registry,
    _Ext(CamelCase),
}

/// indicates which architecture layer manages this dependency or if management is external
/// to starlane itself.  Managing entails: downloading, installing and starting the [StatusProbe]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Manager {
    /// the [StatusProbe] is managed by Starlane's Foundation.  For example a running Starlane
    /// local development cluster might use DockerDesktopFoundation to provide services like
    /// Postgres.
    Foundation,

    /// The [StatusProbe] is managed by the Platform. Again a postgres example: instead of
    /// being responsible for starting Postgres imagine that the [StatusProbe] is actually
    /// a `PostgresConnectionPool` ... The Platform is responsible for creating and maintaining
    /// and status reporting on the health of the connection pool (which isn't the same as
    /// Foundation which is responsible for making the service available and in a ready state)
    Platform,

    /// A completely  external entity manages this [StatusProbe]
    External,
}

/// A [`Provider`] is an add-on to the [`Foundation`] infrastructure which may need to be
/// downloaded, installed, initialized and started.
///
/// The Dependency facilitates instances via ['Provider'].  In other words if the Dependency
/// is a Database server like Postgres... the Dependency will download, install, initialize and
/// start the service whereas a Provider in this example would represent an individual Database
#[async_trait]
pub trait Provider: BaseSub<Config:config::ProviderConfig>+ StatusProbe + EntityReadier + Send + Sync {


    /*
    /// Returns an interface clone for [Provider::Entity] when it reaches [Status::Ready].
    ///
    /// If [Provider::Entity] is NOT ready [Provider::ready] will start the `readying` tasks
    /// and will not return until the [Status::Ready] state is reached or if a [ProviderErr]
    /// is encountered.
    ///
    /// The [Provider::ready] should be reentrant--meaning it can be called multiple times without
    /// causing an error. A [Provider::ready] implementation should always first call
    /// [Provider::probe] to determine the last completed successful [Stage] and continue its
    /// remaining stages if possible.
    ///
    /// Calling [Provider::ready] on a [Provider] that's current [StateDetail]'s variant is
    /// [StateDetail::Pending] should `un-panic` the [Provider] and cause it to retry readying
    /// [Provider::Entity]. A [Provider::ready] invocation on a [Provider] that is
    /// [StateDetail::Fatal] should fail immediately.
    ///
    /// Progress [Status] of [Self::ready] can be tracked using: [Self::status_watcher]
    async fn ready(&self) -> status::ReadyResult<Self::Entity>;

     */
}

/*


/// Query the state of the concrete resource that this [Provider] is modeling
/// and make the [Provider] model match the real world state of said resource.
///
/// [Provider::probe] is especially useful when it comes to updating [StatusEntity::status]
/// from [Status::Unknown]
///
/// The return [Status] may differ between [Provider] that share a [ProviderKind]
/// when the pair are part of the [Platform] and [Foundation] layers respectively.
/// [Platform] [Provider] can only connect to a service or resource via the network, or filesystem.
/// therefore [Provider::probe] may return [Status::Unreachable] which may not be very helpful
/// since the core problem could exist anywhere from the local host to a blocked response from
/// the requester's routing table. Since the [Foundation] [Provider] is capable of `managing`
/// the external service or resource, it can usually provide a more accurate [Status].
/// ```
/// # use std::sync::Arc;
/// # use starlane::provider::{Provider, ProviderKindDef};
/// # use starlane::provider::err::ProviderErr;
/// # use starlane_space::particle::Status;
/// # use starlane_space::status::{StatusDetail, StatusEntity, StatusWatcher};
///
/// struct MyProvider;
///
/// impl StatusEntity for MyProvider {
///   fn status(&self) -> Status {
/// # todo!()
///    }
///
/// fn status_detail(&self) -> StatusDetail {
///         todo!()
///     }
///
/// fn status_watcher(&self) -> StatusWatcher {
///         todo!()
///     }
///
/// async fn probe(&self) -> starlane_space::status::Status {
///         todo!()
///     }}
///
/// impl Provider for MyProvider
/// # { type Config = (); type Item = (); fn kind(&self) -> ProviderKindDef { todo!() }; fn config(&self) -> Arc<Self::Config> { todo!() } async fn ready(&self) -> Result<Self::Item, ProviderErr> {  todo!() } }
/// ```
///



 */
