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

}