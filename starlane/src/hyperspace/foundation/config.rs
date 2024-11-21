use crate::hyperspace::foundation::{DependencyKind, FoundationErr, FoundationKind, Kind, ProviderKind};
use serde_yaml::Value;
use std::hash::Hash;
use serde::{Deserialize, Serialize};

pub type FoundationConfig = Config<FoundationKind,FoundationSubConfig>;
pub type ProtoFoundationConfig = Config<FoundationKind,Value>;

pub type DependencyConfig = Config<DependencyKind,DependencySubConfig>;
pub type ProtoDependencyConfig = Config<DependencyKind,Value>;

pub type ProviderConfig = Config<ProviderKind,ProviderSubConfig>;
pub type ProtoProviderConfig = Config<ProviderKind,Value>;


pub struct FoundationSubConfig {

}


pub struct DependencySubConfig {

}

pub struct ProviderSubConfig {

}

pub trait ProtoConfig {
    fn parse<K,S>(self, expect: K) -> Result<Config<K,S>,FoundationErr>;
}



#[derive(Clone, Serialize, Deserialize)]
pub struct Config<K,C> where K: Kind+Clone, C: Clone{
    pub kind: K,
    pub config: C
}

impl <K> ProtoConfig for Config<K,Value> {
    fn parse<K, S>(self, expect: K) -> Result<Config<K, S>,FoundationErr> {
            if self.kind != expect {
                Err(FoundationErr::foundation_err(FoundationKind::DockerDesktop,format!("expected FoundationKind::{} found FoundationKind::{}", expect, self.kind)))?;
            }

            let sub = serde_yaml::from_value(self.config.clone()).map_err(|err|FoundationErr::foundation_conf_err(self.kind.clone(),err,self.config))?;

            Ok(Self{
                kind: self.kind,
                config: sub
            })
        }
    }
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