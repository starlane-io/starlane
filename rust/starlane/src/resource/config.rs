use crate::artifact::{ArtifactAddress, ArtifactRef};
use crate::cache::Data;
use crate::error::Error;
use crate::resource::{ResourceType, ResourceKind};


pub trait ResourceConfig {
    fn kind(&self)->ResourceKind;
}

pub trait FromArtifact {
    fn artifact(&self)-> ArtifactRef;
    fn references(&self) ->Vec<ArtifactRef>;
}

pub trait Parser<J: FromArtifact> : Send+Sync+'static {
    fn parse(&self, artifact: ArtifactAddress, data: Data ) -> Result<J,Error>;
}
