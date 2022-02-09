use std::sync::Arc;
use mesh_portal_serde::version::latest::bin::Bin;

use crate::artifact::ArtifactRef;
use crate::cache::{Cacheable};
use crate::error::Error;
use crate::resource::Kind;


pub trait Parser<J: Cacheable>: Send + Sync + 'static {
    fn parse(&self, artifact: ArtifactRef, data: Bin) -> Result<Arc<J>, Error>;
}
