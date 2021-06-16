use crate::keys::DomainKey;
use crate::resource::{ResourceAddress, ResourceKind};
use std::convert::{TryInto, TryFrom};
use std::sync::Arc;
use crate::error::Error;
use serde::{Serialize,Deserialize};
use crate::cache::{Data, Cacheable};
use crate::artifact::{ArtifactAddress, ArtifactRef, ArtifactKind};
use std::collections::HashMap;
use crate::resource::config::{FromArtifact, Parser, ResourceConfig};

pub struct Domain{
    key: DomainKey,
    address: ResourceAddress,
}

#[derive(Clone,Serialize,Deserialize)]
pub struct DomainState {
}


impl DomainState{
    pub fn new() -> Self {
        DomainState{
        }
    }
}

impl TryInto<Vec<u8>> for DomainState {

    type Error = Error;

    fn try_into(self) -> Result<Vec<u8>, Self::Error> {
        Ok(bincode::serialize(&self)?)
    }
}

impl TryInto<Arc<Vec<u8>>> for DomainState {

    type Error = Error;

    fn try_into(self) -> Result<Arc<Vec<u8>>, Self::Error> {
        Ok(Arc::new(bincode::serialize(&self)?))
    }
}

impl TryFrom<Arc<Vec<u8>>> for DomainState{
    type Error = Error;

    fn try_from(value: Arc<Vec<u8>>) -> Result<Self, Self::Error> {
        Ok(bincode::deserialize::<DomainState>(value.as_slice() )?)
    }
}

impl TryFrom<Vec<u8>> for DomainState{
    type Error = Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        Ok(bincode::deserialize::<DomainState>(value.as_slice() )?)
    }
}



pub struct HttpResourceSelector{

}


pub struct DomainConfig {
    artifact: ArtifactAddress,
    routes: HashMap<String,HttpResourceSelector>
}

impl Cacheable for DomainConfig {

}

impl FromArtifact for DomainConfig{
    fn artifact(&self) -> ArtifactRef{
        ArtifactRef{
            artifact: self.artifact.clone(),
            kind: ArtifactKind::DomainConfig
        }
    }

    fn references(&self) -> Vec<ArtifactRef> {
        vec!()
    }
}

impl ResourceConfig for DomainConfigParser{
    fn kind(&self) -> ResourceKind {
        ResourceKind::Domain
    }
}


pub struct DomainConfigParser;

impl DomainConfigParser{
    pub fn new()->Self{
        Self{}
    }
}

impl Parser<DomainConfig> for DomainConfigParser{
    fn parse(&self, artifact: ArtifactAddress, data: Data) -> Result<DomainConfig, Error> {
        Ok(DomainConfig{
            artifact: artifact,
            routes: HashMap::new()
        })
    }
}

