use crate::base::config::BaseSubConfig;
use crate::base::provider::{Provider, ProviderKind};

/// root trait definition for[Provider::Config] must implement
pub trait ProviderConfig: BaseSubConfig {}
