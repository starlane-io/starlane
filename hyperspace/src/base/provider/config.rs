use crate::base::kinds;
use crate::base::provider::{Provider, ProviderKindDef};

/// root trait definition for[Provider::Config] must implement
pub trait ProviderConfig {
    type Kind: kinds::ProviderKind+?Sized;

    fn kind(&self) -> &Self::Kind;
}
