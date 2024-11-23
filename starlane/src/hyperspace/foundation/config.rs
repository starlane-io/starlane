use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use crate::hyperspace::foundation::err::FoundationErr;
use crate::hyperspace::foundation::kind::{FoundationKind, IKind};
use crate::hyperspace::foundation::traits::Foundation;


pub type RawConfig = Value;

pub struct Metadata<K> where K: IKind {
    pub kind: K,
}

#[derive(Debug, Clone,Serialize,Deserialize,Eq,PartialEq)]
pub struct ProtoFoundationConfig {
    foundation: FoundationKind,
    config: Value,
}

impl ProtoFoundationConfig {
    pub fn new( foundation: FoundationKind, config: Value ) -> Self {
        Self {
            foundation,
            config
        }
    }
    pub fn create<C>(self) -> Result<FoundationConfig<C>,FoundationErr> {
        Ok(FoundationConfig {
            foundation: self.foundation,
            config: serde_yaml::to_value(self.config).map_err(|e| e.into())?
        })
    }
}

#[derive(Debug, Clone, Serialize,Eq,PartialEq)]
pub struct FoundationConfig<C> where C: Serialize+Eq+PartialEq {
    foundation: FoundationKind,
    config: C
}


pub type DockerCompose = serde_yaml::Value;

#[derive(Debug, Clone,Serialize,Deserialize,Eq,PartialEq)]
pub struct DockerDesktopFoundationConfig {
    compose: DockerCompose
}

impl DockerDesktopFoundationConfig {
}
