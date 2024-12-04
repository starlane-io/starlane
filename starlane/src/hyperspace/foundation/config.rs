use crate::hyperspace::foundation::err::FoundationErr;
use crate::hyperspace::foundation::kind::{
    DependencyKind, FoundationKind, IKind, Kind, ProviderKind,
};
use crate::hyperspace::foundation::util::{  IntoSer, SerMap};
use crate::hyperspace::foundation::{Dependency, Foundation, Provider};
use crate::space::parse::CamelCase;
use derive_name::Name;
use serde;
use serde::de::{DeserializeOwned, DeserializeSeed, Error, MapAccess};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_yaml::Value;
use std::collections::HashMap;
use std::hash::Hash;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use downcast_rs::{impl_downcast, DowncastSync};

pub type RawConfig = Value;

/*
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Metadata<'a,K> where K: Serialize+Deserialize<'a>+'a{
    pub kind: K,
    pub name: Option<String>,
    pub description: Option<String>,
    phantom: PhantomData<&'a ()>,
}

 */

pub trait Config
where
    Self: Sized + SerMap + Name,
    Self::PlatformConfig: PlatformConfig,
    Self::FoundationConfig: FoundationConfig,
{
    type PlatformConfig;
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

impl_downcast!(sync FoundationConfig assoc DependencyConfig);


pub trait DependencyConfig: DowncastSync {
    type ProviderConfig: ProviderConfig+Clone;

    fn kind(&self) -> &DependencyKind;

    fn volumes(&self) -> HashMap<String, String>;

    fn require(&self) -> Vec<Kind>;
}


impl_downcast!(sync DependencyConfig assoc ProviderConfig);

pub trait ProviderConfigSrc
{
    type Config;
    fn providers(&self) -> Result<HashMap<CamelCase, Self::Config>, FoundationErr>;

    fn provider(&self, kind: &CamelCase) -> Result<Option<&Self::Config>, FoundationErr>;
}

pub trait ProviderConfig: DowncastSync {
    fn kind(&self) -> &ProviderKind;
}

impl_downcast!(sync ProviderConfig);


/*
pub trait RegistryConfig: Send+Sync{
   fn create( config: Map ) -> Result<Box<dyn RegistryConfig>,FoundationErr>;

   fn provider(&self) -> &ProviderKind;

}

 */

pub trait PlatformConfig {}

pub(super) mod private {
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
/*
impl <K,C> IntoSer for ConfigMap<K,C> where K: Eq+PartialEq+Hash+Clone+Serialize, C: Clone+IntoSer{
    fn into_ser(&self) -> Box<SerMapDef<Box<dyn SerMap>>> {
        Box::new(SerMapDef::new(self.map.clone().into_iter().map(|(key,value)| (key,value.into_ser())).into_iter().collect::<HashMap<K,Box<dyn SerMap>>>().map(|map|SerMapDef::new(map))))
    }
}

 */

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

/*
impl <'de,K,C> Deserialize<'de> for ConfigMap<K,C> where K: Eq+PartialEq+Hash+Clone+Deserialize<'de>, C: Clone+?Deserialize<'de>{
    fn deserialize<D>(deserializer: D) -> Result<ConfigMap<K,C>, D::Error>
    where
        D: Deserializer<'de>
    {
        let map: HashMap<K,Box<dyn DesMap>> = Deserialize::deserialize(deserializer.clone()).map(deserializer).map_err(D::Error::custom)??;
        let map = map.into_iter().map(DesMap::to_config_map).collect();
        Self {
            map
        }
    }
}

 */

/*
impl <'de,K,C> Deserialize<'de> for ConfigMap<K,C> where K: Eq+PartialEq+Hash+Clone+Deserialize<'de>, C: Clone+Deserialize<'de>{
    fn deserialize<D>(deserializer: D) -> Result<ConfigMap<K,C>, D::Error>
    where
        D: Deserializer<'de>
    {
        deserializer.deserialize_map(ConfigMapVisitor {phantom: PhantomData})
    }
}

 */

/*
impl <'z,C> Deserialize<'z> for C where C: Clone+Deserialize<'z>{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'z>
    {
        Deserialize::deserialize(deserializer).map(C::deserialize)
    }
}

 */

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


pub mod default {
    use std::sync::Arc;

    pub type FoundationConfig = Arc<dyn super::FoundationConfig<DependencyConfig=DependencyConfig>>;
    pub type DependencyConfig = Arc<dyn super::DependencyConfig<ProviderConfig=ProviderConfig>>;

    pub type ProviderConfig= Arc<dyn super::ProviderConfig>;

}