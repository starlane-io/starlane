use std::cell::Cell;
use crate::err::Err;
use crate::src::Source;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;
use std::process;
use wasmer::{Module, Store};

pub trait CacheFactory {
    fn create( & self, source: Box<dyn Source>) -> Box<dyn WasmModuleCache>;
}

pub struct WasmModuleMemCacheFactory {
}

impl WasmModuleMemCacheFactory {
    pub fn new( ) -> Self {
        Self { }
    }
}
impl CacheFactory for WasmModuleMemCacheFactory {

    fn create( & self, source: Box<dyn Source>) -> Box<dyn WasmModuleCache> {
        Box::new(WasmModuleMemCache {
            source,
            map: Default::default(),
        })
    }
}

#[async_trait]
pub trait WasmModuleCache {
    async fn get(&mut self, key: &str, store: & Store) -> Result<Module, Err>;
}

pub struct WasmModuleMemCache {
    source: Box<dyn Source>,
    map: HashMap<String, Result<Module, Err>>,
}
impl WasmModuleMemCache {
    pub fn new( source: Box<dyn Source> ) -> Self {
        Self {
            source,
            map: Default::default(),
        }
    }

}

#[async_trait]
impl WasmModuleCache for WasmModuleMemCache {
    async fn get(&mut self, key: &str, store: & Store) -> Result<Module, Err> {
        if !self.map.contains_key(key) {
            let wasm_bytes = self.source.get(key).await?;
            let module = Module::new(store, wasm_bytes).map_err(|e| e.into());
            self.map.insert(key.to_string(), module);
        }

        match self.map.get(key).unwrap() {
            Ok(module) => Result::Ok(module.clone()),
            Err(err) => Result::Err(err.clone()),
        }
    }
}

impl WasmModuleMemCache {}
