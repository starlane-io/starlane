use crate::resource::{ArtifactAddress, ResourceKind, ResourceAddress, ArtifactKind};
use crate::artifact::ArtifactRef;
use crate::cache::{Cacheable, Data};
use crate::resource::config::{ResourceConfig, Parser};
use std::sync::Arc;
use crate::error::Error;
use std::str::FromStr;
use std::convert::TryInto;

pub struct Wasm {
    pub artifact: ArtifactAddress,
}

impl Cacheable for Wasm {
    fn artifact(&self) -> ArtifactRef {
        ArtifactRef {
            address: self.artifact.clone(),
            kind: ArtifactKind::Wasm,
        }
    }

    fn references(&self) -> Vec<ArtifactRef> {
        vec![]
    }
}

pub struct WasmParser;

impl WasmParser {
    pub fn new() -> Self {
        Self {}
    }
}

impl Parser<Wasm> for WasmParser {
    fn parse(&self, artifact: ArtifactRef, _data: Data) -> Result<Arc<Wasm>, Error> {
        Ok(Arc::new(Wasm {
            artifact: artifact.address,
        }))
    }
}




