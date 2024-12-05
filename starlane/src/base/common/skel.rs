/// # Resource [base::kind::Kind] **Common** Definition
///  A starter template for defining the `common` properties and behaviors of a new
/// [`DependencyKind`](crate::base::kind::DependencyKind)s and/or [`ProviderKind`](crate::base::kind::ProviderKind)s.
///
/// ## !Foundation implementation required!
/// For a new [`Kind`] to work it will also need a concrete implementation for one or more
/// [`Foundation`](crate::base::foundation::Foundation)s. See foundation starter template: [`skel`](crate::base::foundation::skel)
use crate::base;
use base::foundation;

pub trait Dependency: foundation::Dependency<Config:DependencyConfig, Provider: Provider> { }
pub trait Provider: foundation::Provider<Config: ProviderConfig>{ }

pub trait DependencyConfig: foundation::config::DependencyConfig { }
pub trait ProviderConfig: foundation::config::ProviderConfig { }

/// instead of the [foundation::skel::concrete] where the traits defs end and the real
/// implementation begins, when defining a [`common`] for a child resource (Dependency or Provider),
/// [dependency] and [dependency::provider] mods should make the final definition for all the
/// particular `variety` and `mode` abstractions which are relevant to the particular resource
/// being defined yet lacking specific definitions that any particular [foundation::Foundation]
/// requires.
///
/// A good example of this is the `Postgres` common definition which should implement every
/// aspect needed to maybe `initialize` and certainly to `connect` to the Postgres Cluster instance.
/// ```
/// pub mod postgres {
///   use starlane::base::foundation;
///   use starlane::base::foundation::config;
///
///   /// this example implementation is not configured for `modes`.
///   /// a mode implementation example is documented here: [starlane::base::mode]
///   ///
///   /// As you can see this overloaded example provides everything needed to set up a Postgres
///   /// cluster except for any information pertaining to installing and starting the service, because
///   /// the mechanisms for installing and starting differ amongst Foundation implementations.
///   ///
///   /// `DockerDaemonFoundation` implements a concrete version of this `Postgres` dependency
///   /// that inherits the common interface but also adds contextual foundation definitions
///   /// such as a Docker `repository`:`image`:`tag` to be pulled, instanced, initialized and started
///   pub trait DependencyConfig: config::DependencyConfig {
///     fn url(&self) -> String;
///     fn port(&self) -> u16;
///     fn data_directory(&self) -> String;
///     fn username(&self) -> String;
///     fn password(&self) -> String;
///   }
/// }
/// ````
///
pub mod dependency {
    pub mod my { pub use super::super::*; }

    pub mod provider {

    }

}
    /*
pub mod provider {
    use super::my;

    pub mod mode {
        use crate::base::foundation;

        use super::my;
        pub mod create {
            use super::my;
            use super::utilize;
            ///  [`Create`] mode must also [`Utilize`] mode's properties since the foundation
            /// will want to Create the Provision (potentially meaning: downloading, instancing, credential setup,  initializing...etc.)
            /// and then will want to [`Utilize`] the Provision (potentially meaning: authenticating via the same credentials supplied from
            /// [`Create`], connecting to the same port that was set up etc.
            pub trait ProviderConfig: my::ProviderConfig + crate::base::config::provider::mode::utilize::ProviderConfig {}
        }

        pub mod utilize {
            use super::my;
            /// provide any necessary configuration properties to use this Provider after it has been created... etc.
            pub trait ProviderConfig: my::ProviderConfig{

            }

        }

    }
    }
     */


/*
pub mod concrete {
    ///  reference the above a [`my`] implementation ...
    pub mod my { pub use super::super::*; }
    pub use crate::base::foundation;



    pub mod variant {
        use crate::base::foundation;
        use super::my;

        impl foundation::Dependency for Dependency {}
        impl my::Dependency for Dependency {}

        /// [super::variant] follows the same pattern as [`super::variant`] except in this case it is for
        /// [crate::base::foundation::Provider] variants
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
                use super::my;
                use super::utilize;
                ///  [`Create`] mode must also [`Utilize`] mode's properties since the foundation
                /// will want to Create the Provision (potentially meaning: downloading, instancing, credential setup,  initializing...etc.)
                /// and then will want to [`Utilize`] the Provision (potentially meaning: authenticating via the same credentials supplied from
                /// [`Create`], connecting to the same port that was set up etc.
                pub trait ProviderConfig: my::ProviderConfig + crate::base::config::provider::mode::utilize::ProviderConfig {}
            }

            pub mod utilize {
                use super::my;
                pub trait ProviderConfig: my::ProviderConfig {}
            }
        }
    }
}

 */