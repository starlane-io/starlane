use crate::base::config::ProviderConfig;
use crate::base::Platform;

pub trait ProviderKind: Send+Sync {
    type Config: ProviderConfig;
}

pub trait FoundationKind : Send+Sync {}
