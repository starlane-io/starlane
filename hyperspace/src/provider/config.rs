use crate::provider::{Provider, ProviderKindDef};

/// trait definition that [Provider::Config] must implement
pub trait ProviderConfig {
    fn kind(&self) -> &ProviderKindDef;
}