use std::collections::HashMap;
use itertools::Itertools;
use crate::hyperspace::foundation::err::FoundationErr;
use crate::hyperspace::foundation::kind::{DependencyKind, FoundationKind, IKind, ProviderKind};
use crate::hyperspace::foundation::traits::{Dependency, Foundation};
use crate::hyperspace::foundation::util::Map;
use serde::{Deserialize, Serialize};
use serde::de::DeserializeOwned;
use serde_yaml::Value;

pub type RawConfig = Value;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Metadata<K> where K: IKind {
    pub kind: K,
    pub name: Option<String>,
    pub description: Option<String>,
}

impl <K> Metadata<K> where K: IKind {
}


pub trait Config<K>
   where K: IKind,
         Self::PlatformConfig: PlatformConfig,
         Self::FoundationConfig: FoundationConfig,

{
    type PlatformConfig;
    type FoundationConfig;

    fn kind(&self) -> &K;
}

pub trait FoundationConfig {
    fn kind(&self) -> &FoundationKind;

    fn dependency_kinds(&self) -> &Vec<DependencyKind>;

    fn dependency(&self, kind: &DependencyKind) -> Option<&impl DependencyConfig>;

    fn create_dependencies(&self, deps: Vec<Value> ) -> Result<impl DependencyConfig,FoundationErr>;
}

pub trait DependencyConfig: Serialize+DeserializeOwned{
    fn kind(&self) -> &DependencyKind;

    fn volumes(&self) -> Vec<String>;

    fn provider_kinds(&self) ->  Vec<ProviderKind>;

    fn provider(&self, kind: &ProviderKind) -> Option<&impl ProviderConfig>;
}



pub trait ProviderConfig {
    fn kind(&self) -> &ProviderKind;
}


pub trait RegistryConfig {
   fn create( config: Map ) -> Result<impl RegistryConfig,FoundationErr>;

   fn provider(&self) -> &ProviderKind;

}

pub trait PlatformConfig {

}


