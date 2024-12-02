use crate::hyperspace::foundation::err::FoundationErr;
use crate::hyperspace::foundation::kind::{DependencyKind, FoundationKind, IKind, ProviderKind};
use crate::hyperspace::foundation::util::Map;
use serde::{Deserialize, Serialize};
use serde::de::DeserializeOwned;
use serde_yaml::Value;
use crate::hyperspace::foundation::{Dependency, Foundation};

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

pub trait FoundationConfig: Clone+Sized {
    fn kind(&self) -> &FoundationKind;

    fn dependency_kinds(&self) -> &Vec<DependencyKind>;

    fn dependency(&self, kind: &DependencyKind) -> Option<&impl DependencyConfig>;
}

pub trait DependencyConfig: Clone+Serialize+DeserializeOwned{
    fn kind(&self) -> &DependencyKind;

    fn volumes(&self) -> Vec<String>;

    fn provider_kinds(&self) ->  Vec<ProviderKind>;

    fn provider(&self, kind: &ProviderKind) -> Option<&impl ProviderConfig>;
}



pub trait ProviderConfig {
    fn kind(&self) -> &ProviderKind;
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
