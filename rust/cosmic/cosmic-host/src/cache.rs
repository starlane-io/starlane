use crate::err::Err;
use crate::src::WasmSource;
use async_trait::async_trait;
use std::collections::HashMap;
use wasmer::{Module, Store};


#[async_trait]
pub trait WasmModuleCache
{
    async fn get(&self, key: &str ) -> Result<Module, Err>;
}

pub struct WasmModuleMemCache {
    map: HashMap<String, Module>,
    src: Box<dyn WasmSource>,
}


#[async_trait]
impl WasmModuleCache for WasmModuleMemCache {
    async fn get(&self, key: &str) -> Result<Module, Err> {
        let wasm_bytes = self.src.get(key).await?;
        println!("Compiling module...");
        let store = Store::default();
        let module = Module::new(&store,wasm_bytes).unwrap();
        Ok(module)
    }
}

impl WasmModuleMemCache {
    pub fn new( src: Box<dyn WasmSource>) -> Self {
        Self {
            map: Default::default(),
            src
        }
    }
}