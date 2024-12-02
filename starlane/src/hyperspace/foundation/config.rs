use std::collections::HashMap;
use crate::hyperspace::foundation::err::FoundationErr;
use crate::hyperspace::foundation::kind::{DependencyKind, FoundationKind, IKind, Kind, ProviderKind};
use crate::hyperspace::foundation::util::Map;
use serde::{Deserialize, Serialize};
use serde::de::DeserializeOwned;
use serde_yaml::Value;
use crate::hyperspace::foundation::{Dependency, Foundation};
use crate::space::parse::CamelCase;

pub type RawConfig = Value;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Metadata<K> where K: IKind {
    pub kind: K,
    pub name: Option<String>,
    pub description: Option<String>,
}

impl <K> Metadata<K> where K: IKind {
}




pub trait Config
   where Self: Sized,
         Self::PlatformConfig: PlatformConfig,
         Self::FoundationConfig: FoundationConfig,

{
    type PlatformConfig;
    type FoundationConfig;

    fn foundation(&self) -> Self::FoundationConfig;
    fn platform(&self) -> Self::FoundationConfig;
}

pub trait FoundationConfig{
    fn kind(&self) -> &FoundationKind;

    /// required [`Vec<Kind>`]  must be installed and running for THIS [`Foundation`] to work.
    /// at a minimum this must contain a Registry of some form.
    fn required(&self) -> &Vec<Kind>;

    fn dependency_kinds(&self) -> &Vec<DependencyKind>;

    fn dependency(&self, kind: &DependencyKind) -> Option<&Box<dyn DependencyConfig>>;

    fn clone_me(&self) -> Box<dyn FoundationConfig>;
}

pub trait DependencyConfig {
    fn kind(&self) -> &DependencyKind;

    fn volumes(&self) -> &HashMap<String,String>;

    fn require(&self) -> &Vec<Kind>;

    fn providers(&self) ->  &HashMap<CamelCase,Box<dyn ProviderConfig>>;

    fn provider(&self, kind: &ProviderKind) -> Option<Box<dyn ProviderConfig>>;

    fn clone_me(&self) -> Box<dyn DependencyConfig>;
}



pub trait ProviderConfig{
    fn kind(&self) -> &ProviderKind;

    fn clone_me(&self) -> Box<dyn ProviderConfig>;
}


pub trait RegistryConfig: Clone+Sized {
   fn create( config: Map ) -> Result<impl RegistryConfig,FoundationErr>;

   fn provider(&self) -> &ProviderKind;

}

pub trait PlatformConfig {

}


pub(super) mod private {
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
}
