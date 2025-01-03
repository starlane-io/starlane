pub use crate::config;

pub trait FoundationConfig: config::FoundationConfig<ProviderConfig: ProviderConfig> {}

pub trait ProviderConfig: config::ProviderConfig {}
