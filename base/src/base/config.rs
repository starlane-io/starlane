    use crate::err::BaseErr;
    use std::collections::HashSet;
    use std::hash::Hash;
    use crate::provider::Provider;
    use crate::ProviderKind;
    use crate::kind::FoundationKind;
    use crate::Foundation;
    use crate::Platform;

    use crate::provider;
    use starlane_hyperspace as hyperspace;
    use starlane_hyperspace::registry;

    /// a container for all sub-strata layers
    pub trait BaseConfig
    {
        type Err: Into<BaseErr>;
        type PlatformConfig: PlatformConfig + ?Sized;
        type FoundationConfig: FoundationConfig + ?Sized;

        fn foundation(&self) -> Self::FoundationConfig;
        fn platform(&self) -> Self::PlatformConfig;
    }


    /// [CommonBaseConfig] for implementing any common traits for both [PlatformConfig] and
    /// [FoundationConfig]
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


    pub trait ProviderConfig: provider::config::ProviderConfig  { }

    pub trait PlatformConfig: hyperspace::base::PlatformConfig  {}


    pub trait RegistryConfig: registry::RegistryConfig { }


