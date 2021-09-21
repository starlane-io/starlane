use crate::resource::{ResourceKind, ResourceAddress, ArtifactKind};
use crate::artifact::ArtifactRef;
use crate::cache::{Cacheable, Data};
use crate::resource::config::{ResourceConfig, Parser};
use std::sync::Arc;
use crate::error::Error;
use std::str::FromStr;
use std::convert::TryInto;
use wasmer::{Cranelift, Universal, Store, Module};
use starlane_resources::ResourcePath;

pub struct Wasm {
    pub artifact: ResourcePath,
    pub module: Arc<Module>
}

impl Cacheable for Wasm {
    fn artifact(&self) -> ArtifactRef {
        ArtifactRef {
            path: self.artifact.clone(),
            kind: ArtifactKind::Wasm,
        }
    }

    fn references(&self) -> Vec<ArtifactRef> {
        vec![]
    }
}

pub struct WasmCompiler {
    store: Store
}

impl WasmCompiler {
    pub fn new() -> Self {
        let engine = Universal::new(Cranelift::default()).engine();
        let store = Store::new(&engine);
        Self {store}
    }
}

impl Parser<Wasm> for WasmCompiler{
    fn parse(&self, artifact: ArtifactRef, data: Data) -> Result<Arc<Wasm>, Error> {

       let module = Arc::new(Module::new( &self.store, data.as_ref() )?);
       Ok(Arc::new(Wasm{
            artifact: artifact.path,
            module
        }))
    }
}




