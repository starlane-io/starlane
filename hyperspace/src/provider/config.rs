use crate::provider::ProviderKindDef;
use crate::provider::Provider;

/// trait definition that [Provider::Config] must implement
pub trait ProviderConfig {
    fn kind(&self) -> &ProviderKindDef;
}