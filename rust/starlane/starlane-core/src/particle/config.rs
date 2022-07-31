use mesh_portal::version::latest::bin::Bin;
use std::sync::Arc;

use crate::artifact::ArtifactRef;
use crate::cache::Cacheable;
use crate::error::Error;

pub trait Parser<J: Cacheable>: Send + Sync + 'static {
    fn parse(&self, artifact: ArtifactRef, data: Bin) -> Result<Arc<J>, Error>;
}
