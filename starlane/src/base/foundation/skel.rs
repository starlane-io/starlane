use ariadne::Config;
use crate::base::foundation;


/// [`foundation::skel`] provides a starter custom implementation of a [`crate::base::foundation`]
/// here you can see the recommended extension technique of re-implementing each of the traits
/// and then providing a [`concrete`] child mod where the concrete implementations of the
/// same suite of APIs are implemented,
///
/// please copy this file to a new mod and customize as needed


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