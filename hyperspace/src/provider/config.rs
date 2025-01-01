use crate::provider::ProviderKindDef;

pub trait ProviderConfig {
    fn kind(&self) -> &ProviderKindDef;
}