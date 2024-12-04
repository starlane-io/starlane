use std::collections::HashMap;
pub use crate::base;

pub trait FoundationConfig: base::config::FoundationConfig<DependencyConfig:DependencyConfig> {}

pub trait DependencyConfig: base::config::DependencyConfig<ProviderConfig:ProviderConfig> {
    fn volumes(&self) -> HashMap<String, String>;
}

pub trait ProviderConfig: base::config::ProviderConfig{}
