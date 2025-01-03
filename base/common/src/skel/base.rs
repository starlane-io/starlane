/// NOTE: enable feature `skel` in your IDE for a better learning experience...
///
/// # COMMON?
/// This example further defines traits for specific [Provider] implementation. The
/// trait extensions are said to be `base` because these trait extensions will be implemented
/// in the [foundation] and [platform] with a matching [ProviderKind].
///
/// The `base` implementation pattern only makes sense if you understand different jobs
/// that [Platform] and [Foundation] fulfil. To put it simply: [Platform] enables `utilization` of
/// services and resources that are not native to `Starlane` and the [Foundation] enables
/// `creation` and `management` of those same external services or resources.
///
/// So for example the [Platform] [config::ProviderConfig] implementation for
/// [ProviderKind::PostgresService] holds the postgres connection pool config
/// including: hostname, port and credentials.  And the [DockerDaemonFoundation] [ProviderConfig]
/// implementation for [ProviderKind::PostgresService] requires the same connection pool
/// info because the [DockerDaemonFoundation]'s Postgres [Provider] implementation needs
/// to provision using the credentials that the [Platform] expects.

/// ## !Foundation implementation required!
/// For a new [Provider] to work it will also need a concrete implementation for one or more
/// [Foundation](crate::Foundation)s. See foundation starter template: [`skel`](crate::base::foundation::skel)

use crate::platform;
use crate::provider;
use crate::foundation;
use crate::kind::ProviderKind;
use crate::config;
use crate::common;

pub trait Provider: provider::Provider<Config: ProviderConfig> {}

pub trait ProviderConfig: foundation::config::ProviderConfig {}

/// Unlike the implementations defined in [foundation::skel::concrete] where the final `structs`
/// are implementing the entire inheritance tree for their type, `base` merely defines child
/// traits.
///
/// [provider] mods should make the final definition for all the
/// particular `variety` and `mode` abstractions which are relevant to the particular resource
/// being defined yet lacking specific definitions that any particular [foundation::Foundation]
/// requires.
///
/// A good example of this is the `Postgres` base definition which should implement every
/// aspect needed to maybe `initialize` and certainly to `connect` to the Postgres Cluster instance.
/// ```
/// pub mod postgres {
///
///   use starlane::config;
/// use starlane_base::config;
///
///   /// this example implementation is not configured for `modes`.
///   /// a mode implementation example is documented here: [starlane::base::mode]
///   ///
///   /// As you can see this overloaded example provides everything needed to set up a Postgres
///   /// cluster except for any information pertaining to installing and starting the service, because
///   /// the mechanisms for installing and starting differ amongst Foundation implementations.
///   ///
///   /// `DockerDaemonFoundation` implements a concrete version of this `Postgres` dependency
///   /// that inherits the base interface but also adds contextual foundation definitions
///   /// such as a Docker `repository`:`image`:`tag` to be pulled, instanced, initialized and started
///   pub trait ProviderConfig: config::ProviderConfig{
///     fn url(&self) -> String;
///     fn port(&self) -> u16;
///     fn data_directory(&self) -> String;
///     fn username(&self) -> String;
///     fn password(&self) -> String;
///   }
/// }
/// ````
///




pub mod concrete {
    ///  reference the above a [`my`] implementation ...
    pub mod my { pub use super::super::*; }
    pub use crate::foundation;



    pub mod variant {
        use crate::foundation;
        use super::my;

        /// [super::variant] follows the same pattern as [`super::variant`] except in this case it is for
        /// [crate::common::foundation::Provider] variants
        pub mod variant {
            use super::my;
            pub struct Provider {}
            impl my::Provider for Provider {}
        }
    }


    pub mod provider {
        use super::my;
        pub mod mode {
            use super::my;
            pub mod create {
                use crate::config::ProviderMode;
                use super::my;
                use super::utilize;
                use crate::Provider;
                ///  [ProviderMode::Control] mode must also contain [ProviderMode::Utilize] mode's
                /// config since the foundation will want to Create the Provision (potentially
                /// meaning: downloading, instancing, credential setup,  initializing...etc.)
                /// and then will want to [ProviderMode::Utilize] the [Provider] (potentially meaning:
                /// authenticating via the same credentials supplied from
                /// [ProviderMode::Control], connecting to the same port that was set up etc.
                pub trait ProviderConfig: my::ProviderConfig + utilize::ProviderConfig {}
            }

            pub mod utilize {
                use super::my;
                pub trait ProviderConfig: my::ProviderConfig {}
            }
        }
    }
}
