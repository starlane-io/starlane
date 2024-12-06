use crate::base;

pub trait PlatformConfig: base::config::BaseConfig {}

pub trait DependencyConfig: base::config::DependencyConfig<ProviderConfig: ProviderConfig> {}

pub trait ProviderConfig: base::config::ProviderConfig {}
