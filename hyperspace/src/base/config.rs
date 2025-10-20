use crate::base::provider::Provider;
use crate::base::provider::ProviderKindDisc;
use std::collections::HashSet;
use std::hash::Hash;

use crate::base::err::BaseErr;
use crate::base::{provider, BaseSub};
use crate::registry;

/// a container for all sub-strata layers
pub trait BaseConfig: Send + Sync {
    type Err: Into<BaseErr>;
    type PlatformConfig: PlatformConfig + ?Sized;
    type FoundationConfig: FoundationConfig + ?Sized;

    fn foundation(&self) -> Self::FoundationConfig;
    fn platform(&self) -> Self::PlatformConfig;
}

//
pub trait BaseSubConfig: Send + Sync {}

pub trait FoundationConfig: BaseSubConfig {
    fn required(&self) -> HashSet<ProviderKindDisc>;

    fn provider_kinds(&self) -> &HashSet<ProviderKindDisc>;

    /*
    fn provider<P>(&self, kind: <Self as BaseSubConfig>::Kind) -> Option<&P> where P: Provider+BaseSub<Config: BaseSubConfig>;

     */
}

pub trait ProviderConfig: provider::config::ProviderConfig {}

pub trait PlatformConfig: crate::base::PlatformConfig {}

pub trait RegistryConfig: registry::RegistryConfig {}
