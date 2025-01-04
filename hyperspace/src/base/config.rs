    use std::collections::HashSet;
    use std::hash::Hash;
    use crate::base::provider::Provider;
    use crate::base::provider::ProviderKind;

    use crate::base::{kinds, provider};
    use crate::registry;

    /// a container for all sub-strata layers
    pub trait BaseConfig: Send+Sync
    {
        type Err: Into<BaseErr>;
        type PlatformConfig: PlatformConfig + ?Sized;
        type FoundationConfig: FoundationConfig + ?Sized;

        fn foundation(&self) -> Self::FoundationConfig;
        fn platform(&self) -> Self::PlatformConfig;
    }


    /// [CommonBaseConfig] for implementing any common traits for both [PlatformConfig] and
    /// [FoundationConfig]
    pub trait CommonBaseConfig: Send+Sync { }

        pub trait FoundationConfig: CommonBaseConfig {
            type ProviderConfig: ProviderConfig + ?Sized;

            fn kind(&self) -> & Self::P

            /// required [HashSet<ProviderKind>]  must be installed and running for THIS [`Foundation`] to work.
            /// at a minimum this must contain a Registry of some form.
            fn required(&self) -> HashSet<ProviderKind>;

            fn provider_kinds(&self) -> &HashSet<ProviderKind>;

            fn provider(&self, kind: &ProviderKind) -> Option<&Self::ProviderConfig>;
        }



    pub trait ProviderConfig: provider::config::ProviderConfig  { }

    pub trait PlatformConfig: crate::base::PlatformConfig {}


    pub trait RegistryConfig: registry::RegistryConfig { }


