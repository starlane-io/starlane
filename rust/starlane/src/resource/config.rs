use crate::artifact::{ArtifactAddress, ArtifactRef};
use crate::cache::{Data, Cacheable};
use crate::error::Error;
use crate::resource::{ResourceType, ResourceKind};
use std::sync::Arc;


pub trait ResourceConfig {
    fn kind(&self)->ResourceKind;
}


pub trait Parser<J: Cacheable> : Send+Sync+'static {
    fn parse(&self, artifact: ArtifactRef, data: Data ) -> Result<Arc<J>,Error>;
}
