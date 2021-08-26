use crate::resource::{ArtifactAddress, ResourceKind};
use crate::cache::{Cacheable, Data};
use std::collections::HashMap;
use crate::resource::ArtifactKind;
use crate::artifact::ArtifactRef;
use crate::resource::config::{ResourceConfig, Parser};
use std::sync::Arc;
use crate::error::Error;

pub struct AppConfig {
    artifact: ArtifactAddress
}

impl Cacheable for AppConfig {
    fn artifact(&self) -> ArtifactRef {
        ArtifactRef {
            address: self.artifact.clone(),
            kind: ArtifactKind::AppConfig,
        }
    }

    fn references(&self) -> Vec<ArtifactRef> {
        vec![]
    }
}

impl ResourceConfig for AppConfigParser {
    fn kind(&self) -> ResourceKind {
        ResourceKind::App
    }
}

pub struct AppConfigParser;

impl AppConfigParser {
    pub fn new() -> Self {
        Self {}
    }
}

impl Parser<AppConfig> for AppConfigParser {
    fn parse(&self, artifact: ArtifactRef, _data: Data) -> Result<Arc<AppConfig>, Error> {
        Ok(Arc::new(AppConfig {
            artifact: artifact.address,
        }))
    }
}