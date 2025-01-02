pub use crate::base;
use std::collections::HashMap;

pub trait FoundationConfig: base::config::FoundationConfig<ProviderConfig: ProviderConfig> {}

pub trait ProviderConfig: base::config::ProviderConfig {}
