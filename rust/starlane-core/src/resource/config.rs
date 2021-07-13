use crate::artifact::ArtifactRef;
use crate::resource::ArtifactAddress;
use crate::cache::{Cacheable, Data};
use crate::error::Error;
use crate::resource::{ResourceKind, ResourceType};
use std::sync::Arc;

pub trait ResourceConfig {
    fn kind(&self) -> ResourceKind;
}

pub trait Parser<J: Cacheable>: Send + Sync + 'static {
    fn parse(&self, artifact: ArtifactRef, data: Data) -> Result<Arc<J>, Error>;
}
