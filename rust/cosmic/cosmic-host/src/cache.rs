use crate::err::Err;
use crate::src::Source;
use async_trait::async_trait;
use std::collections::HashMap;
use std::process;
use wasmer::{Module, Store};

pub trait CacheFactory {
    fn create( self, store: &Store) -> Box<dyn WasmModuleCache>;
}

pub struct WasmModuleMemCacheFactory {
   source: Box<dyn Source>
}

impl WasmModuleMemCacheFactory {
    pub fn new( source: Box<dyn Source>) -> Self {
        Self {
            source
        }
    }
}
impl CacheFactory for WasmModuleMemCacheFactory {
    fn create<'a>( self, store: & 'a Store) -> Box<dyn WasmModuleCache> {
        Box::new(WasmModuleMemCache {
            store,
            map: Default::default(),
            source: self.source,
        })
    }
}

#[async_trait]
pub trait WasmModuleCache {
    async fn get(&mut self, key: &str) -> Result<Module, Err>;
}

pub struct WasmModuleMemCache<'a> {
    store: &'a Store,
    map: HashMap<String, Result<Module, Err>>,
    source: Box<dyn Source>,
}

#[async_trait]
impl<'a> WasmModuleCache for WasmModuleMemCache<'a> {
    async fn get(&mut self, key: &str) -> Result<Module, Err> {
        if !self.map.contains_key(key) {
            let wasm_bytes = self.src.get(key).await?;
            let module = Module::new(self.store, wasm_bytes).map_err(|e| e.into());
            self.map.insert(key.to_string(), module);
        }

        match self.map.get(key).unwrap() {
            Ok(module) => Result::Ok(module.clone()),
            Err(err) => Result::Err(err.clone()),
        }
    }
}

impl<'a> WasmModuleMemCache<'a> {}
