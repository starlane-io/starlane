use crate::artifact::ArtifactRef;
use crate::cache::Cacheable;
use crate::error::Error;
use crate::particle::config::Parser;
use cosmic_api::id::ArtifactSubKind;
use mesh_portal::version::latest::bin::Bin;
use mesh_portal::version::latest::id::Point;
use std::convert::TryInto;
use std::str::FromStr;
use std::sync::Arc;
use wasmer::{Cranelift, Module, Store, Universal};

pub struct Wasm {
    pub artifact: Point,
    pub module: Arc<Module>,
}

impl Cacheable for Wasm {
    fn artifact(&self) -> ArtifactRef {
        ArtifactRef {
            point: self.artifact.clone(),
            kind: ArtifactSubKind::Wasm,
        }
    }

    fn references(&self) -> Vec<ArtifactRef> {
        vec![]
    }
}

pub struct WasmCompiler {
    store: Store,
}

impl WasmCompiler {
    pub fn new() -> Self {
        let engine = Universal::new(Cranelift::default()).engine();
        let store = Store::new(&engine);
        Self { store }
    }
}

impl Parser<Wasm> for WasmCompiler {
    fn parse(&self, artifact: ArtifactRef, data: Bin) -> Result<Arc<Wasm>, Error> {
        let module = Arc::new(Module::new(&self.store, data.as_ref())?);
        Ok(Arc::new(Wasm {
            artifact: artifact.point,
            module,
        }))
    }
}
