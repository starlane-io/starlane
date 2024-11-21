use crate::hyperspace::foundation::{DependencyKind, FoundationKind, Kind, ProviderKind};
use serde_yaml::Value;
use std::hash::Hash;
use serde::{Deserialize, Serialize};

pub type FoundationConfig = Config<FoundationKind,FoundationSubConfig>;
pub type ProtoFoundationConfig = Config<FoundationKind,Value>;

pub type DependencyConfig = Config<DependencyKind,DependencySubConfig>;
pub type ProtoDependencyConfig = Config<DependencyKind,Value>;

pub type ProviderConfig = Config<ProviderKind,ProviderSubConfig>;
type ProtoProviderConfig = Config<ProviderKind,Value>;


pub struct FoundationSubConfig {

}


pub struct DependencySubConfig {

}

pub struct ProviderSubConfig {

}



#[derive(Clone, Serialize, Deserialize)]
struct Config<K,C> where K: Kind+Clone, C: Clone{
    pub kind: K,
    pub config: C
}

trait KindDeserializer where Self::Kind: Kind{
    type Kind;
}


impl <K,C> Config<K,C> where K: Kind{
    fn new(kind: K, config: C) -> Self {
        Self {
            kind,
            config
        }
    }
}