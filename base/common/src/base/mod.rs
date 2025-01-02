
/// # Starlane `Base`
/// The Starlane architecture is composed of multiple layers:
///
/// * [Space](starlane_space) -- APIs and utilities for driving and extending `Starlane` with
///   [Particle]'s ([Particle] is an abstract enterprise resource in Starlane parlance).
///   An enterprise's  `Value Adding Code` can be developed in to run in the [Space](starlane_space)
///   layer which is designed to minimize or eliminate the friction of writing infrastructure
///   code.
///
/// * [HyperSpace](starlane_hyperspace) -- APIs and utilities which provide the magical
///   infrastructure that supports the [Space](starlane_space) layer.
///   [HyperSpace](starlane_hyperspace) facilitates communication between [Particle]/s enforces
///   security and type safety and can extend the [Space](starlane_space) layer with [Driver]
///   implementations for new kinds of [Particle].
///
/// * [Base](crate::base) -- A support layer which provides starlane with non-native functionality
///   that can also be extended through the use of [Provider] (which is an abstract trait defined in
///   [HyperSpace](starlane_hyperspace), yet implemented almost exclusively int the base layers.
///    [Base](crate::base) is actually comprised of two layers: [Platform] and [Foundation]
///
/// * [Platform] -- A layer that supplies [Provider]'s which understand how to connect,
///   communicate and utilize external non-native support elements for Starlane.  For example:
///   [Platform] may have a [Provider] implementation for a Postgres Service (or cluster).
///   The [Provider::Config] implementation for `PostgresServiceProvider` contains the cluster.
///
///   Invoking `PostgresServiceProvider's` [Provider::ready] method should return a
///   [Handle<PostgresServiceStub>] which contains a database connection pool.
///
///  * [Foundation] -- Starlane's lowest architectural layer.  When a user installs a new Starlane
///    Context he must select a specific [Foundation] implementation.  For local development
///    the [DockerDaemonFoundation] is recommended (and at the time of this writing the only
///    [Foundation] implementation available!).
///
///    So why is the [Foundation] needed and how does it differ from [Platform]'s role? You remember
///    that the [Platform] layer can create connection pools to external services... a more abstract
///    way to think of it is that the [Platform]'s [Provider]'s can `utilize` external services, yet
///    it does not `manage` anything.
///
///    [Foundation] level [Provider] implementations actively `manage` the lifecycle of non-native
///    services that the [Platform] layer depends on.
///
///    If we go back to our Postgres Service example... the [Platform] PostgresService [Provider]
///    implementation can specify the [Foundation]'s PostgresService [Provider] as a dependency, and
///    in the case of [DockerDaemonFoundation]'s PostgresService [Provider] a postgres docker
///    image will be pulled and run (including coordinating port exposure and local persistent
///    volume mounts and assigning the same credentials that the [Platform] expects...). When the
///    Docker container is probed and determined to be [Status::Ready] the [Foundation] will
///    yield control to the [Platform] layer which will hastily create a connection pool with
///    the brand, new Postgres.
///
///  `IMPORTANT:` if the user wants Starlane to use an existing Postgres instance all he has
///   supply the connection info and credentials in [Platform::Config] and simply omit the
///   [Foundation] dependency.  The important concept to grasp is that the Base layers provide
///   a separation between `utilization` [Platform] , and `management` [Foundation]. Because
///   of this separation of concerns the same [Platform::Config] that was created for the
///   development environment be used when deployed to production. Let's say, for example,
///   the production environment's foundation implementation is [KubernetesFoundation] (not
///   yet available but hopefully someday!) .... When the new Starlane configuration is
///   deployed to production [Platform] request [ProviderKind::PostgresService] [Status::Ready]
///   state and [KubernetesFoundation] provisions a production grade Postgres setup (including
///   PgBounce for connections, replica sets, read/write masters and slaves... etc.

use crate::base::platform::prelude::Platform;
use crate::base::foundation::Foundation;
use starlane_hyperspace::driver::Driver;
use starlane_space::particle::Particle;
use starlane_hyperspace::provider::Provider;

use downcast_rs::Downcast;
use partial::config::PartialConfig;
use starlane_space::status::Handle;

pub mod foundation;
pub mod config;

pub mod common;


pub mod platform;

pub mod err;
pub mod registry;
pub mod partial;
pub mod mode;
pub mod provider;
mod kind;

