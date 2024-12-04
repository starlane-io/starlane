use std::hash::Hash;
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use downcast_rs::{impl_downcast, DowncastSync};
use crate::base;
use crate::base::err::{BaseErr, BaseErrBuilder};
use crate::base::foundation::kind::FoundationKind;
use crate::base::foundation::Provider;
use crate::base::kind::{DependencyKind, Kind, ProviderKind};
use crate::hyperspace::driver::base::Base;
use crate::space::parse::CamelCase;

pub trait Config
where
    Self::PlatformConfig: PlatformConfig,
    Self::FoundationConfig: FoundationConfig,
{
    type Err: Into<BaseErr>;
    type PlatformConfig: PlatformConfig;
    type FoundationConfig: FoundationConfig+Clone;

    fn foundation(&self) -> Self::FoundationConfig;
    fn platform(&self) -> Self::FoundationConfig;
}


pub trait FoundationConfig: DowncastSync {
    type DependencyConfig: DependencyConfig+Clone;

    fn kind(&self) -> FoundationKind;

    /// required [`Vec<Kind>`]  must be installed and running for THIS [`Foundation`] to work.
    /// at a minimum this must contain a Registry of some form.
    fn required(&self) -> Vec<Kind>;

    fn dependency_kinds(&self) -> &Vec<DependencyKind>;

    fn dependency(&self, kind: &DependencyKind) -> Option<&Self::DependencyConfig>;

}

pub trait DependencyConfig: DowncastSync {
    type ProviderConfig: ProviderConfig+Clone;

    fn kind(&self) -> &DependencyKind;

    fn require(&self) -> Vec<Kind>;
}

pub trait ProviderConfigSrc
{

    type Config: ProviderConfig;


    fn providers(&self) -> Result<HashMap<CamelCase, Self::Config>, BaseErr>;

    fn provider(&self, kind: &CamelCase) -> Result<Option<&Self::Config>, BaseErr>;
}

pub trait ProviderConfig: DowncastSync {
    fn kind(&self) -> &ProviderKind;
}


pub enum ProviderMode<C,U> where C: provider::mode::create::ProviderConfig, U: C+provider::mode::utilize::ProviderConfig {
    Create(C),
    Utilize(U)
}


pub mod provider {
    use super as config;
    pub mod mode {
        use super::config;
        pub mod create {
            use super::config;
            use super::utilize;
            ///  [`Create`] mode must also [`Utilize`] mode's properties since the foundation
            /// will want to Create the Provision (potentially meaning: downloading, instancing, credential setup,  initializing...etc.)
            /// and then will want to [`Utilize`] the Provision (potentially meaning: authenticating via the same credentials supplied from
            /// [`Create`], connecting to the same port that was set up etc.
            pub trait ProviderConfig: config::ProviderConfig+utilize::ProviderConfig { }
        }

        pub mod utilize{
            use super::config;
            pub trait ProviderConfig: config::ProviderConfig{
            }
        }

    }
}


pub trait PlatformConfig {}

pub(crate) mod private {
    /*
    pub struct Config {
        foundation: <Self as super::Config>::FoundationConfig,
        platform: <Self as super::Config>::PlatformConfig,
    }

    impl super::Config for Config {
        type PlatformConfig = ();
        type FoundationConfig = ();

        fn foundation(&self) -> Self::FoundationConfig {
            todo!()
        }

        fn platform(&self) -> Self::FoundationConfig {
            todo!()
        }
    }

     */
}

/// Implement the Downcast's
impl_downcast!(sync FoundationConfig assoc DependencyConfig);

impl_downcast!(sync DependencyConfig assoc ProviderConfig);

impl_downcast!(sync ProviderConfig);

/*
#[derive(Clone)]
pub struct ConfigMap<K, C>
where
    K: Eq + PartialEq + Hash + Clone,
    C: Clone+?Sized
{
    map: HashMap<K, C>,
}

impl<K, C> Default for ConfigMap<K, C>
where
    K: Default + Eq + PartialEq + Hash + Clone,
    C: Default + Clone,
{
    fn default() -> Self {
        let map = HashMap::default();
        Self { map }
    }
}

impl<K, C> ConfigMap<K, C>
where
    K: Eq + PartialEq + Hash + Clone,
    C: Clone,
{
    pub fn new() -> ConfigMap<K, C> {
        ConfigMap {
            map: HashMap::default(),
        }
    }

    pub fn from(map: HashMap<K, C>) -> Self {
        Self { map }
    }

    pub fn add(&mut self, kind: K, config: C) {
        self.map.insert(kind, config);
    }

    pub fn transform<'z, F, K2, C2>(&self, factory: F) -> ConfigMap<K2, C2>
    where
        F: Fn((K, C)) -> (K2, C2),
        K2: Deserialize<'z> + Eq + PartialEq + Hash + Clone,
        C2: Deserialize<'z> + Clone,
    {
        ConfigMap::from(
            self.map
                .clone()
                .into_iter()
                .map(factory)
                .into_iter()
                .collect(),
        )
    }
}

impl<K, C> Serialize for ConfigMap<K, C>
where
    K: Eq + PartialEq + Hash + Clone + Serialize,
    C: Serialize + Clone,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.map.len()))?;
        for (k, v) in self.map.clone().iter() {
            map.serialize_entry(k, v)?;
        }
        map.end()
    }
}

impl<K, C> Deref for ConfigMap<K, C>
where
    K: Eq + PartialEq + Hash + Clone,
    C: Clone,
{
    type Target = HashMap<K, C>;

    fn deref(&self) -> &Self::Target {
        &self.map
    }
}

impl<K, C> DerefMut for ConfigMap<K, C>
where
    K: Eq + PartialEq + Hash + Clone,
    C: Clone,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.map
    }
}

 */

pub mod default {
    use std::sync::Arc;

    pub type FoundationConfig = Arc<dyn super::FoundationConfig<DependencyConfig=DependencyConfig>>;
    pub type DependencyConfig = Arc<dyn super::DependencyConfig<ProviderConfig=ProviderConfig>>;

    pub type ProviderConfig= Arc<dyn super::ProviderConfig>;

}

/// this is the super trait of [`foundation::config::FoundationConfig`] and [`platform::config::PlatformConfig`]
pub trait BaseConfig  {

    type DependencyConfig: DependencyConfig<ProviderConfig:ProviderConfig>;

}
