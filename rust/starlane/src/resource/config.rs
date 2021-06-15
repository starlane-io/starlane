use crate::artifact::Artifact;
use crate::cache::Data;
use crate::error::Error;
use crate::resource::{ResourceType, ResourceKind};


pub trait ResourceConfig {
    fn kind(&self)->ResourceKind;
}

pub trait FromArtifact {
    fn artifact(&self)-> Artifact;
    fn dependencies(&self)->Vec<Artifact>;
}

pub trait Parser<J: FromArtifact> : Send+Sync+'static {
    fn parse(&self, artifact: Artifact, data: Data ) -> Result<J,Error>;
}
