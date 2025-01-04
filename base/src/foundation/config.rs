pub use starlane_hyperspace::base::config;

pub trait FoundationConfig: config::FoundationConfig<ProviderConfig: ProviderConfig> {}

pub trait ProviderConfig: config::ProviderConfig {}
