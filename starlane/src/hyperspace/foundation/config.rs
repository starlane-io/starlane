use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;
use derive_name::Name;
use crate::hyperspace::foundation::err::FoundationErr;
use crate::hyperspace::foundation::kind::{DependencyKind, FoundationKind, IKind, Kind, ProviderKind};
use crate::hyperspace::foundation::util::{DesMap, SerMap};
use serde::{Deserialize, Serialize};
use serde::de::DeserializeOwned;
use serde_yaml::Value;
use crate::hyperspace::foundation::{Dependency, Foundation, Provider};
use crate::space::parse::CamelCase;

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

pub trait FoundationConfig: Send+Sync+SerMap{
    fn kind(&self) -> &FoundationKind;

    /// required [`Vec<Kind>`]  must be installed and running for THIS [`Foundation`] to work.
    /// at a minimum this must contain a Registry of some form.
    fn required(&self) -> &Vec<Kind>;

    fn dependency_kinds(&self) -> &Vec<DependencyKind>;

    fn dependency(&self, kind: &DependencyKind) -> Option<&Arc<dyn DependencyConfig>>;

    fn clone_me(&self) -> Arc<dyn FoundationConfig>;
}



pub trait DependencyConfig: Send+Sync{
    fn kind(&self) -> &DependencyKind;

    fn volumes(&self) -> HashMap<String,String>;

    fn require(&self) -> Vec<Kind>;

    fn clone_me(&self) -> Arc<dyn DependencyConfig>;
}

impl IntoConfigTrait for dyn DependencyConfig {
    type Config = dyn DependencyConfig;
}

pub trait IntoConfigTrait {
    type Config;
    fn into_trait(self) -> Arc<Self::Config> {
        let config = Arc::new(self);
        config as Arc<Self::Config>
    }
}

pub trait ProviderConfigSrc<P>: DependencyConfig where P: ProviderConfig{
    fn providers(&self) ->  Result<&HashMap<CamelCase,P>,FoundationErr>;

    fn provider(&self, kind: &CamelCase) -> Result<Option<&P>,FoundationErr>;
}


pub trait ProviderConfig: Send+Sync{
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
