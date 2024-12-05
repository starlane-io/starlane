use crate::base;
use base::foundation;


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


/// we create this trait just in case we need to custom traits for this partial with this feature
pub trait Partial: base::partial::Partial { }

pub trait PartialConfig: base::partial::config::PartialConfig{}

pub mod partial {
    /// here is the continued implementation of the `mount` partial defined here: [partial::skel]
    use crate::base;
    use base::err;
    use base::partial;
    use partial::skel as mount;
    use base::foundation;

    /// here we add a few odds and ends for the [Mounts] partial that are
    /// required for this particular [foundation::Foundation]
    pub trait MountsConfig: mount::MountsConfig {
        /// Returns the `$user` to own child Volumes
        fn owner(&self) -> String;

        /// Returns the `$permissons` to be set for child Volumes in octal
        fn permissons(&self) -> u16;

    }
    #[async_trait]
    pub trait Mounts: mount::Mounts<Config: MountsConfig,Volume:Volume> {

        /// the concrete implementation of Mounts for this Foundation
        /// will call [Mounts::chown] which will set the permission sof all child volumes
        /// to `$permissions` where the value of `$permission` be the octal value return
        /// of [MountsConfig::permissons]
        /// ```bash
        /// chmod $permissions $volume
        /// ```
        async fn chown(&self) -> Result<(),err::BaseErr>;
        async fn chmod(&self) -> Result<(),err::BaseErr>;


    }
    pub trait VolumeConfig: mount::VolumeConfig{ }
    pub trait Volume: mount::Volume<Config: MountsConfig> { }
}


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


    pub mod partial {
        use super::my;
        pub mod mounts {
            use super::my;

        }
    }

}