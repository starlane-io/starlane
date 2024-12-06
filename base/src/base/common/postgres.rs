use crate::base;
use crate::space::parse::DbCase;
use base::foundation;

/// Dependency and Providers of a particular Kind usually share some common traits regardless
/// of any Foundation affiliation. For example: multiple foundations define a dependency for
/// `Postgres` (and at the time of this writing `Postgres` *is* the only Registry Foundation implemented.
/// A `common` trait definition for postgres might look like this:

/// Here we define the common config api for a Postgres Cluster instance
pub trait DependencyConfig: base::config::DependencyConfig {
    /// define the Postgres port
    fn port(&self) -> u16;

    /// the `root` username for this Postgres Cluster
    fn username(&self) -> String;

    /// um... hrm... well, Starlane should get this from a Secrets Vault... but that isn't implemented
    /// so for now it's provided in plaintext
    fn password(&self) -> String;

    /// provide Postgres' actual persistent storage volume. Should default to: `/var/lib/postgresql/data`
    fn volume(&self) -> String;
}

pub trait Dependency: foundation::Dependency<Config: DependencyConfig, Provider: Provider> {}
///

/// here we define the common attributes and api for every Postgres Provider (which in the case of Postgres is an actual Database in the Dependency cluster we created)
pub trait ProviderConfig: base::config::ProviderConfig {
    /// the name of the database to create in the parent dependency postgres cluster.
    /// notice it uses `DbCase` which is a String implementation that enforces SQL nameing rules (mixed snake case... yes -> [ "my_database", "I_am_Your_Database"]  no -> ["no-Hyphens Spaces_or_!#%Weird_Characters!"]
    fn database(&self) -> DbCase;
}

/// define the common Mode's of a Postgres Provider
pub mod mode {}


pub mod provider {
    //    use crate::base::common::postgres as my;
    mod my {
        pub use super::*;
    }

    pub mod mode {
        use super::my;
        pub mod create {
            use super::my;
            ///  [`Create`] mode must also [`Utilize`] mode's properties since the foundation
            /// will want to Create the Provision (potentially meaning: downloading, instancing, credential setup,  initializing...etc.)
            /// and then will want to [`Utilize`] the Provision (potentially meaning: authenticating via the same credentials supplied from
            /// [`Create`], connecting to the same port that was set up etc.
            pub trait ProviderConfig: my::ProviderConfig + my::provider::mode::utilize::ProviderConfig {}
        }

        pub mod utilize {
            pub trait ProviderConfig: crate::base::config::ProviderConfig {}
        }
    }
}
///
///
/// /// create a variant of
///
///

/// [`common::skel`] provides

pub trait Provider: foundation::Provider<Config: ProviderConfig> {}

pub trait FoundationConfig: foundation::config::FoundationConfig<DependencyConfig: DependencyConfig> {}


pub mod concrete {
    ///  reference the above a [`my`] implementation ...
    ///
    mod my {
        pub use super::*;
    }

    pub mod variant {
        use super::my;

        pub struct Dependency {}
        impl my::Dependency for Dependency {}

        /// [super::variant] follows the same pattern as [`super::variant`] except in this case it is for
        /// [crate::base::foundation::Provider] variants
        pub mod variant {
            use super::my;
            pub struct Provider {}
            impl my::Provider for Provider {}
        }
    }
}

