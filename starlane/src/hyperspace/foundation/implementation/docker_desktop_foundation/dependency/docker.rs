use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use crate::hyperspace::foundation::config::ProviderConfig;
use crate::hyperspace::foundation::kind::{DependencyKind, ProviderKind};
use crate::space::parse::CamelCase;

#[derive(Debug,Clone,Eq,PartialEq,Serialize,Deserialize)]
pub struct DockerProviderConfig{
    kind: ProviderKind,
    image: String,
    expose: HashMap<u16,u16>
}

impl DockerProviderConfig{
    pub fn new(kind: ProviderKind, image: String, expose: HashMap<u16,u16>) -> DockerProviderConfig{
        Self {
            kind,
            image,
            expose,
        }
    }
}

impl ProviderConfig for DockerProviderConfig{
    fn kind(&self) -> &ProviderKind {
        &self.kind
    }
}