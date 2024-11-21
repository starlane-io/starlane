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


#[derive(Clone, Serialize, Deserialize)]
pub struct FoundationSubConfig {

}


#[derive(Clone, Serialize, Deserialize)]
pub struct DependencySubConfig {

}

#[derive(Clone, Serialize, Deserialize)]
pub struct ProviderSubConfig {

}

pub trait ProtoConfig where Self::Kind: Kind+Clone {
    type Kind;
    fn parse<S>(self, expect: Self::Kind) -> Result<Config<Self::Kind,S>,FoundationErr> where S: Clone;
}



#[derive(Clone, Serialize, Deserialize)]
pub struct Config<K,C> where K: Kind+Clone, C: Clone{
    pub kind: K,
    pub config: C
}

impl <K> ProtoConfig for Config<K,Value> where K: Kind+Clone{
    type Kind = K;

    fn parse<S>(self, expect: Self::Kind) -> Result<Config<Self::Kind, S>,FoundationErr> where S:Clone{
        /*
            if self.kind != expect {
                Err(FoundationErr::foundation_err(FoundationKind::DockerDesktop,format!("expected FoundationKind::{} found FoundationKind::{}", expect, self.kind)))?;
            }

            let sub = serde_yaml::from_value(self.config.clone()).map_err(|err|FoundationErr::foundation_conf_err(self.kind.clone(),err,self.config))?;

            Ok(Self{
                kind: self.kind,
                config: sub
            })
         */
        todo!()
        }

    }

trait KindDeserializer where Self::Kind: Kind{
    type Kind;
}


impl <K,C> Config<K,C> where K: Kind, C: Clone{
    fn new(kind: K, config: C) -> Self {
        Self {
            kind,
            config
        }
    }
}

pub mod sub {


}


#[cfg(test)]
pub mod test {
   #[test]
   pub fn test() {

   }
}