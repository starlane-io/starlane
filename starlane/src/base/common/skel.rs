use crate::base::foundation;

/// Dependency and Providers of a particular Kind usually share some common traits regardless
/// of any Foundation affiliation. For example: multiple foundations define a dependency for
/// `Postgres` (and at the time of this writing `Postgres` *is* the only Registry Foundation implemented.
/// A `common` trait definition for postgres might look like this:
/// ```
/// use std::collections::HashMap;
/// use starlane::base;
/// use base::foundation;use starlane::base::foundation::util::Map;use starlane::space::parse::DbCase;
///
/// /// Here we define the common config api for a Postgres Cluster instance
/// pub trait DependencyConfig: base::config::DependencyConfig {
///    /// define the Postgres port
///    fn port(&self) -> u16;
///
///    /// the `root` username for this Postgres Cluster
///    fn username(&self) -> String;
///
///    /// um... hrm... well, Starlane should get this from a Secrets Vault... but that isn't implemented
///    /// so for now it's provided in plaintext
///    fn password(&self) -> String;
///
///    /// provide Postgres' actual persistent storage volume. Should default to: `/var/lib/postgresql/data`
///    fn volume(&self)  -> String;
/// }
///
/// pub trait Dependency: foundation::Dependency<Config:DependencyConfig, Provider: Provider> { }///
///
/// /// here we define the common attributes and api for every Postgres Provider (which in the case of Postgres is an actual Database in the Dependency cluster we created)
/// pub trait ProviderConfig: base::config::ProviderConfig {
///
///    /// the name of the database to create in the parent dependency postgres cluster.
///    /// notice it uses `DbCase` which is a String implementation that enforces SQL nameing rules (mixed snake case... yes -> [ "my_database", "I_am_Your_Database"]  no -> ["no-Hyphens Spaces_or_!#%Weird_Characters!"]
///    fn database(&self) -> DbCase;
/// }
///
///
/// /// since
/// pub mod mode {
///
/// }
///
/// enum ProviderMode{
///    Create(),
///    CONNECT
/// }
///
/// pub trait Provider: foundation::Provider {
///
/// }
///
///

/// [`common::skel`] provides


/// this trait is for extending [`foundation::Foundation`] API and constraining generic traits like [`FoundationConfig`] so
/// that foundation implementations can better customize their traits for whatever is required.
pub trait Foundation: foundation::Foundation<Config:FoundationConfig, Dependency: Dependency, Provider: Provider> { }
pub trait Dependency: foundation::Dependency<Config:DependencyConfig, Provider: Provider> { }
pub trait Provider: foundation::Provider<Config: ProviderConfig>{ }

pub trait FoundationConfig: foundation::config::FoundationConfig<DependencyConfig:DependencyConfig> { }
pub trait DependencyConfig: foundation::config::DependencyConfig { }
pub trait ProviderConfig: foundation::config::ProviderConfig { }


pub mod concrete {
    ///  reference the above a [`my`] implementation ...
    pub(self) use super as my;

    pub struct Foundation {}
    impl my::Foundation for Foundation {}

    /// [super::variant] is just a generic mod name for a [`Dependency`] variant.
    /// when implementing this pattern probably give it a name that differentiates if from
    /// other dependencies.  For example: if the hypothetical implementation is for [`FoundationKind::Kubernetes`]
    /// the various concrete dependency implementations should have meaningful names like: `postgres`,
    /// `keycloak`, `s3`, `kafka` ...  and of course instead of one custom dependency variant
    /// multiple implementations can and should be implemented for this Foundation
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