use std::collections::HashMap;
pub use crate::base;

pub trait FoundationConfig: base::config::FoundationConfig<DependencyConfig:DependencyConfig> { }

pub trait DependencyConfig: base::config::DependencyConfig<ProviderConfig:ProviderConfig> {
    /// the foundation Dependency may require [`DependencyConfig::volumes`] which are basically
    /// directories that need to be provisioned for persisted storage
    fn volumes(&self) -> HashMap<String, String>;
}

pub trait ProviderConfig: base::config::ProviderConfig { }
