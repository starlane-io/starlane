use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;
use derive_name::Name;
use crate::hyperspace::foundation::err::FoundationErr;
use crate::hyperspace::foundation::kind::{DependencyKind, FoundationKind, IKind, Kind, ProviderKind};
use crate::hyperspace::foundation::util::{DesMap, DesMapFactory, IntoSer, Map, SerMap};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde::de::{DeserializeOwned, Error};
use serde_yaml::{Sequence, Value};
use std::ops::{Deref, DerefMut};
use serde::ser::SerializeMap;
use serde_with_macros::serde_as;
use crate::hyperspace::foundation::{Dependency, Foundation, Provider};
use crate::space::parse::CamelCase;
use serde;

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


pub trait Config:
   where
         Self: Sized+ SerMap +Name,
         Self::PlatformConfig: PlatformConfig,
         Self::FoundationConfig: FoundationConfig,

{
    type PlatformConfig;
    type FoundationConfig;

    fn foundation(&self) -> Self::FoundationConfig;
    fn platform(&self) -> Self::FoundationConfig;
}

pub trait FoundationConfig: Send+Sync+IntoSer{
    fn kind(&self) -> &FoundationKind;

    /// required [`Vec<Kind>`]  must be installed and running for THIS [`Foundation`] to work.
    /// at a minimum this must contain a Registry of some form.
    fn required(&self) -> &Vec<Kind>;

    fn dependency_kinds(&self) -> &Vec<DependencyKind>;

    fn dependency(&self, kind: &DependencyKind) -> Option<&Arc<dyn DependencyConfig>>;

    fn clone_me(&self) -> Arc<dyn FoundationConfig>;
}



pub trait DependencyConfig: Send+Sync+IntoSer{
    fn kind(&self) -> &DependencyKind;

    fn volumes(&self) -> HashMap<String,String>;

    fn require(&self) -> Vec<Kind>;

    fn clone_me(&self) -> Arc<dyn DependencyConfig>;
}




pub trait ProviderConfigSrc<P>: DependencyConfig where P: ProviderConfig{
    fn providers(&self) ->  Result<&HashMap<CamelCase,P>,FoundationErr>;

    fn provider(&self, kind: &CamelCase) -> Result<Option<&P>,FoundationErr>;
}


pub trait ProviderConfig: Send+Sync+IntoSer{
    fn kind(&self) -> &ProviderKind;

    fn clone_me(&self) -> Arc<dyn ProviderConfig>;
}


/*
pub trait RegistryConfig: Send+Sync{
   fn create( config: Map ) -> Result<Box<dyn RegistryConfig>,FoundationErr>;

   fn provider(&self) -> &ProviderKind;

}

 */

pub trait PlatformConfig {

}


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
pub struct ConfigMap<K,C> where K: Eq+PartialEq+Hash+Clone, C: Clone, {
    map: HashMap<K,C>,
}

impl <K,C> Default for ConfigMap<K,C> where K: Default+Eq+PartialEq+Hash+Clone, C: Default+Clone, {
    fn default() -> Self {
        let map = HashMap::default();
        Self {
            map
        }
    }
}


impl <K,C> ConfigMap<K,C> where K: Eq+PartialEq+Hash+Clone, C: Clone{
    pub fn new() -> ConfigMap<K,C> {
        ConfigMap {
            map: HashMap::default()
        }
    }

    pub fn add( &mut self, kind: K, config: C) {
        self.map.insert(kind, config);
    }

    pub fn into_ser<K2,C2>(&self) -> ConfigMap<K2,Box<C2>> where K2: Eq+PartialEq+Hash+Clone, C: IntoSer+Clone, C2: SerMap {
        self.map.clone().into_iter().map(|(key,value)| { (key,value.into_ser()) }).collect()
    }
    pub fn transform<F,K2,C2>(&self, factory: F) -> ConfigMap<K2,C2> where F: Fn((K2,C2)) -> Box<dyn DesMap>+Copy, K2: Eq+PartialEq+Hash+Clone, C2: SerMap+Clone {
        self.map.clone().into_iter().map(factory).collect()
    }
}


impl <K,C> IntoSer for ConfigMap<K,C> where K: Eq+PartialEq+Hash+Clone+IntoSer, C: Clone+IntoSer {
    fn into_ser(&self) -> Box<dyn SerMap> {
        Box::new(self.clone()) as Box<dyn SerMap>
    }
}

impl <K,C> Serialize for ConfigMap<K,C> where K: IntoSer, C: IntoSer{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer
    {
        let map = self.map.clone().into_iter().map(|(key,value)| (key.into_ser(), value.into_ser())).collect::<HashMap<Box<dyn SerMap>,Box<dyn SerMap>>>();

        let config = ConfigMap {
            map
        };

        config.serialize(serializer)
    }
}




impl <K,C> Serialize for ConfigMap<K,C> where K: Eq+PartialEq+Hash+Clone+Serialize, C: Serialize{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer
    {

        let mut map = serializer.serialize_map(Some(self.map.len()))?;
        for (k, v) in self.map.clone().into_iter() {
            map.serialize_entry(k, v)?;
        }
        map.end()
    }
}
impl <'de,K,C> Deserialize for ConfigMap<K,C> where K: Eq+PartialEq+Hash+Clone, C: Clone{
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

impl <'de,K,C> Deserialize for ConfigMap<K,C> where K: Eq+PartialEq+Hash+Clone+Deserialize<'de>, C: Clone+DeserializeOwned{
    fn deserialize<D>(deserializer: D) -> Result<ConfigMap<K,C>, D::Error>
    where
        D: Deserializer<'de>
    {
         let map: HashMap<K,Box<dyn DesMap>> = Deserialize::deserialize(deserializer).map(C::deserialize).map_err(D::Error::custom)??;
         let map = map.into_iter().map(DesMap::to_config_map).collect();
         Self {
             map
         }
    }
}

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

impl <K,C> Deref for ConfigMap<K,C> where K: Eq+PartialEq+Hash+Clone{
    type Target = HashMap<K,C>;

    fn deref(&self) -> &Self::Target {
        &self.map
    }
}

impl <K,C> DerefMut for ConfigMap<K,C> where K: Eq+PartialEq+Hash+Clone{
    fn deref_mut(&mut self) -> &mut Self::Target {
        & mut self.map
    }
}