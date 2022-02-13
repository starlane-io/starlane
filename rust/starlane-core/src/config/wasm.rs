use crate::resource::{Kind, ArtifactKind};
use crate::artifact::ArtifactRef;
use crate::cache::Cacheable;
use std::sync::Arc;
use crate::error::Error;
use std::str::FromStr;
use std::convert::TryInto;
use mesh_portal::version::latest::bin::Bin;
use mesh_portal::version::latest::id::Address;
use wasmer::{Cranelift, Universal, Store, Module};
use crate::resource::config::Parser;

pub struct Wasm {
    pub artifact: Address,
    pub module: Arc<Module>
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
    fn parse(&self, artifact: ArtifactRef, data: Bin) -> Result<Arc<Wasm>, Error> {

       let module = Arc::new(Module::new( &self.store, data.as_ref() )?);
       Ok(Arc::new(Wasm{
            artifact: artifact.address,
            module
        }))
    }
}




