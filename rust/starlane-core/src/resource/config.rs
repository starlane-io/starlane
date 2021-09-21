use std::sync::Arc;

use crate::artifact::ArtifactRef;
use crate::cache::{Cacheable, Data};
use crate::error::Error;
use crate::resource::ResourceKind;

pub trait ResourceConfig {
    fn kind(&self) -> ResourceKind;
}

pub trait Parser<J: Cacheable>: Send + Sync + 'static {
    fn parse(&self, artifact: ArtifactRef, data: Data) -> Result<Arc<J>, Error>;
}
