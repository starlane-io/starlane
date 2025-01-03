use crate::err::BaseErr;
use downcast_rs::{Downcast, DowncastSync};
use std::collections::HashSet;
use std::hash::Hash;
use std::ops::{Deref, DerefMut};
use crate::provider::Provider;
use crate::ProviderKind;
use crate::kind::FoundationKind;

pub use starlane_hyperspace::provider;
pub use starlane_hyperspace::provider::config::ProviderConfig;

pub trait BaseConfig
{
    type Err: Into<BaseErr>;
    type PlatformConfig: PlatformConfig + ?Sized;
    type FoundationConfig: FoundationConfig + ?Sized;

    fn foundation(&self) -> Self::FoundationConfig;
    fn platform(&self) -> Self::PlatformConfig;
}


pub trait CommonBaseConfig { }


pub trait FoundationConfig: CommonBaseConfig {
    type ProviderConfig: ProviderConfig+ ?Sized;

    fn kind(&self) -> FoundationKind;

    /// required [HashSet<ProviderKind>]  must be installed and running for THIS [`Foundation`] to work.
    /// at a minimum this must contain a Registry of some form.
    fn required(&self) -> HashSet<ProviderKind>;

    fn provider_kinds(&self) -> &HashSet<ProviderKind>;

    fn provider(&self, kind: &ProviderKind) -> Option<&Self::ProviderConfig>;
}



/*
pub enum ProviderMode<C, U>
where
    C: ::ProviderConfig,
    U: provider::mode::utilize::ProviderConfig,
{
    Utilize(U),
    Control(C),
}

 */

/*

pub mod provider {
    use crate::config as my;
    pub mod mode {
        use super::my;
        pub mod create {
            use super::utilize;
            use super::super::super::ProviderMode;
            use starlane_hyperspace::provider::{config, Provider};
            ///  [ProviderMode::Control] mode must also contain [ProviderMode::Utilize] mode's
            /// properties since the foundation will want to Create the Provision
            /// (potentially meaning: downloading, instancing, credential setup,  initializing...
            /// etc.) and then will want to [ProviderMode::Utilize] the [Provider::Entity] (potentially meaning:
            /// authenticating via the same credentials supplied from [ProviderMode::Control],
            /// connecting to the same port that was set up etc.
            pub trait ProviderConfig: config::ProviderConfig + utilize::ProviderConfig {}
        }

        pub mod utilize {
            pub trait ProviderConfig: config::ProviderConfig {}
        }
    }
}

 */


pub trait PlatformConfig {}


pub mod default {
    mod my { pub use super::super::*; }
    use std::sync::Arc;

    pub type FoundationConfig = Arc<dyn super::FoundationConfig<ProviderConfig=ProviderConfig>>;

    pub type ProviderConfig = Arc<dyn my::ProviderConfig>;
}


