use mesh_portal_serde::version::latest::command::common::SetProperties;
use crate::artifact::ArtifactRef;
use crate::cache::Cacheable;
use crate::command::compose::Command;
use crate::resource::Kind;

pub struct ResourceConfig {
    pub artifact_ref: ArtifactRef,
    pub kind: Kind,
    pub properties: SetProperties,
    pub install: Vec<Command>
}

impl Cacheable for ResourceConfig {
    fn artifact(&self) -> ArtifactRef {
        self.artifact_ref.clone()
    }

    fn references(&self) -> Vec<ArtifactRef> {
        vec![]
    }
}