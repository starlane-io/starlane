use std::hash::Hash;
use crate::base::config::ProviderConfig;
use crate::base::Platform;


pub trait Kind: Send+Sync { }

pub trait RegistryKind: Kind { }
pub trait ProviderKind: Kind { }
pub trait FoundationKind: Kind { }
pub trait PlatformKind: Kind { }

