use crate::artifact::ArtifactResourceAddress;
use crate::cache::Data;
use crate::error::Error;
use crate::resource::{ResourceType, ResourceKind};


pub trait ResourceConfig {
    fn kind(&self)->ResourceKind;
}

pub trait FromArtifact {
    fn artifact(&self)->ArtifactResourceAddress;
}

pub trait Parser<J: FromArtifact> : Send+Sync+'static {
    fn parse( &self, artifact: ArtifactResourceAddress, data: Data ) -> Result<J,Error>;
}
