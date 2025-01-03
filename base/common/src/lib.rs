#![allow(warnings)]

//! # BASE STRATA
//! Trait definitions and abstract support implementations for the first two
//! rungs of `Starlane's` layered  architecture: [Foundation] and [Platform]
//!
//! Starlane `Base Strata` provides `utilization` and `management` of services
//! and resources that are external to and not native of Starlane.
//!
//! A quick overview of Starlane's layer roles:
//!
//! # SUPER STRATA
//! * [Space](starlane_space) -- value added enterprise business logic
//!
//! * [HyperSpace](starlane_hyperspace) -- the `auto infrastructure` facilitator that
//!   supports the [Space](starlane_space)
//!
//! # BASE STATA
//! * [Platform] -- enables `utilization` of non-native, external support entities
//!   example: a connection pool to a database
//!
//! * [Foundation] --  enables `management` of non-native, external support services and resources
//!   meaning [Foundation] can download, install, initialize, start and stop support
//!   entities for the [Platform] to then `utilize`
//!
//!
//! # WHY IS THE BASE STRATA ARCHITECTED LIKE THIS?
//!  Good question! Let's say the user wants Starlane to use an existing Postgres instance. He must
//!  supply the connection info and credentials in [crate::src::platform::prelude::Platform::Config]
//!  and simply omit the [Foundation] dependency.  The important concept to grasp is that the
//!  Base layers provide
//!  a separation between `utilization` [Platform] , and `management` [Foundation]. Because
//!  of this separation of concerns the same [crate::src::platform::prelude::Platform::Config] that
//!  was created for the
//!  development environment be used when deployed to production. Let's say, for example,
//!  the production environment's foundation implementation is [KubernetesFoundation] (not
//!  yet available but hopefully someday!) .... When the new Starlane configuration is
//!  deployed to production [Platform] request [ProviderKind::PostgresService] [Status::Ready]
//!  state and [KubernetesFoundation] provisions a production grade Postgres setup (including
//!  PgBounce for connections, replica sets, read/write masters and slaves... etc.
//
//
//
// * [Space](starlane_space) -- APIs and utilities for driving and extending `Starlane` with
//   [Particle]'s ([Particle] is an abstract enterprise resource in Starlane parlance).
//   An enterprise's  `Value Adding Code` can be developed in to run in the [Space](starlane_space)
//   layer which is designed to minimize or eliminate the friction of writing infrastructure
//   code.
//
// * [HyperSpace](starlane_hyperspace) -- APIs and utilities which provide the magical
//   infrastructure that supports the [Space](starlane_space) layer.
//   [HyperSpace](starlane_hyperspace) facilitates communication between [Particle]/s enforces
//   security and type safety and can extend the [Space](starlane_space) layer with [Driver]
//   implementations for new kinds of [Particle].
//
// * [Base](crate::base) -- A support layer which provides starlane with non-native functionality
//   that can also be extended through the use of [Provider] (which is an abstract trait defined in
//   [HyperSpace](starlane_hyperspace), yet implemented almost exclusively int the base layers.
//    [Base](crate::base) is actually comprised of two layers: [Platform] and [crate::Foundation]
//
// * [Platform] -- A layer that supplies [Provider]'s which understand how to connect,
//   communicate and utilize external non-native support elements for Starlane.  For example:
//   [Platform] may have a [Provider] implementation for a Postgres Service (or cluster).
//   The [Provider::Config] implementation for `PostgresServiceProvider` contains the cluster.
//
//   Invoking `PostgresServiceProvider's` [Provider::ready] method should return a
//   [Handle<PostgresServiceStub>] which contains a database connection pool.
//
//  * [crate::Foundation] -- Starlane's lowest architectural layer.  When a user installs a new Starlane
//    Context he must select a specific [crate::Foundation] implementation.  For local development
//    the [DockerDaemonFoundation] is recommended (and at the time of this writing the only
//    [crate::Foundation] implementation available!).
//
//    So why is the [crate::Foundation] needed and how does it differ from [Platform]'s role? You remember
//    that the [Platform] layer can create connection pools to external services... a more abstract
//    way to think of it is that the [Platform]'s [Provider]'s can `utilize` external services, yet
//    it does not `manage` anything.
//
//    [crate::Foundation] level [Provider] implementations actively `manage` the lifecycle of non-native
//    services that the [Platform] layer depends on.
//
//    If we go back to our Postgres Service example... the [Platform] PostgresService [Provider]
//    implementation can specify the [crate::Foundation]'s PostgresService [Provider] as a dependency, and
//    in the case of [DockerDaemonFoundation]'s PostgresService [Provider] a postgres docker
//    image will be pulled and run (including coordinating port exposure and local persistent
//    volume mounts and assigning the same credentials that the [Platform] expects...). When the
//    Docker container is probed and determined to be [Status::Ready] the [crate::Foundation] will
//    yield control to the [Platform] layer which will hastily create a connection pool with
//    the brand, new Postgres.
//




pub mod foundation;
pub mod config;
pub mod base;
pub mod platform;
pub mod err;
pub mod registry;
pub mod partial;
pub mod mode;
pub mod provider;
pub mod kind;
pub mod status;

use platform::prelude::Platform;
use once_cell::sync::Lazy;
use std::str::FromStr;
use async_trait::async_trait;
use starlane_hyperspace::provider::{Provider, ProviderKind};
use std::sync::Arc;
use starlane_hyperspace::registry::Registry;
use starlane_space::progress::{Progress};
use tokio::sync::watch::Receiver;
use starlane_hyperspace::driver::Driver;
use starlane_space::particle::Particle;
use starlane_space::status::{Handle, StatusWatcher,StatusReporter};
use crate::kind::FoundationKind;
use status::Status;
use status::StatusDetail;
use err::BaseErr;

#[cfg(feature="skel")]
pub(crate) mod skel;


#[cfg(test)]
pub mod test;
// we cannot afford `safety` with prices as high as they are
// pub mod safety;

pub static VERSION: Lazy<semver::Version> =
    Lazy::new(|| semver::Version::from_str(env!("CARGO_PKG_VERSION").trim()).unwrap());



pub fn init() {

    {
        use rustls::crypto::aws_lc_rs::default_provider;
        default_provider()
            .install_default()
            .expect("crypto provider could not be installed");
    }
}

// ['Foundation'] is an abstraction for managing infrastructure.
#[async_trait]
pub trait Foundation: Sync + Send {

    /// [Foundation::Config] should be a `concrete` implementation of [config::FoundationConfig]
    type Config: config::FoundationConfig + ?Sized;

    /// [Foundation::Provider] Should be [Provider] or a custom `trait` that implements [Provider]
    type Provider: Provider + ?Sized;

    fn kind(&self) -> FoundationKind;

    fn config(&self) -> Arc<Self::Config>;

    fn status(&self) -> status::Status;

    async fn status_detail(&self) -> Result<status::StatusDetail, err::BaseErr>;

    fn status_watcher(&self) -> StatusWatcher;

    /// [Foundation::probe] synchronize [Foundation]'s model from that of the external services
    /// and return a [Status].  [Foundation::probe] should also rebuild the [Provider][StatusDetail]
    /// model and update [StatusReporter]
    async fn probe(&self) -> Status;

    /// Take action to bring this [Foundation] to [Status::Ready] if not already. A [Foundation]
    /// is considered ready when all [Provider] dependencies are [Status::Ready].
    async fn ready(&self, progress: Progress) -> Result<(), BaseErr>;

    /// Returns a [Provider] implementation which
    fn provider(&self, kind: &ProviderKind) -> Result<Option<Box<Self::Provider>>, BaseErr>;

}

