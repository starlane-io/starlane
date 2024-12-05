/// # SKEL
///  a starter template for defining the `common` properties and behaviors of
///  new [`DependencyKind`](crate::base::kind::DependencyKind)s and/or [`ProviderKind`](crate::base::kind::ProviderKind)s.
///
/// for a new [`Kind`] to work it will also need a concrete implementation for one or more [`Foundation`](crate::base::foundation::Foundation)s.
/// See  [`skel`](crate::base::foundation::skel)
use crate::base::foundation;

pub mod provider {
    use super as config;

    /// an Example of a Provider with `modes` . It's basically a custom
    pub mod mode {
        use super::config;
        pub mod create {
            use super::config;
            use super::utilize;
            ///  [`Create`] mode must also [`Utilize`] mode's properties since the foundation
            /// will want to Create the Provision (potentially meaning: downloading, instancing, credential setup,  initializing...etc.)
            /// and then will want to [`Utilize`] the Provision (potentially meaning: authenticating via the same credentials supplied from
            /// [`Create`], connecting to the same port that was set up etc.
            pub trait ProviderConfig: crate::base::config::ProviderConfig+ crate::base::config::provider::mode::utilize::ProviderConfig { }
        }

        pub mod utilize{
            use super::config;
            /// provide any necessary configuration properties to use this Provider after it has been created... etc.
            pub trait ProviderConfig: crate::base::config::ProviderConfig{

            }

        }

    }
}///
///
/// /// create a variant of
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


    pub mod provider {
    use super as config;
    pub mod mode {
        use super::config;
        pub mod create {
            use super::config;
            use super::utilize;
            ///  [`Create`] mode must also [`Utilize`] mode's properties since the foundation
            /// will want to Create the Provision (potentially meaning: downloading, instancing, credential setup,  initializing...etc.)
            /// and then will want to [`Utilize`] the Provision (potentially meaning: authenticating via the same credentials supplied from
            /// [`Create`], connecting to the same port that was set up etc.
            pub trait ProviderConfig: crate::base::config::ProviderConfig+ crate::base::config::provider::mode::utilize::ProviderConfig { }
        }

        pub mod utilize{
            use super::config;
            pub trait ProviderConfig: crate::base::config::ProviderConfig{
            }
        }

    }
}///
///
/// /// create a variant of
///
///

/// [`common::skel`] provides


/// this trait is for extending [`foundation::Foundation`] API and constraining generic traits like [`FoundationConfig`] so
/// that foundation implementations can better customize their traits for whatever is required.






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