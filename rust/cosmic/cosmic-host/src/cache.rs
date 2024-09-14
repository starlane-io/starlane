use std::cell::Cell;
use crate::err::Err;
use crate::src::Source;
use async_trait::async_trait;
use std::collections::HashMap;
use std::process;
use wasmer::{Module, Store};

pub trait CacheFactory {
    fn create<'a>( &self, source: Box<dyn Source>, store: & 'a Store) -> Box<dyn WasmModuleCache + 'a>;
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

    fn create<'a>( &self, source: Box<dyn Source>, store: & 'a Store) -> Box<dyn WasmModuleCache + 'a> {
        Box::new(WasmModuleMemCache {
            source,
            store,
            map: Default::default(),
        })
    }
}

#[async_trait]
pub trait WasmModuleCache {
    async fn get(&mut self, key: &str) -> Result<Module, Err>;
}

pub struct WasmModuleMemCache<'a> {
    source: Box<dyn Source>,
    store: &'a Store,
    map: HashMap<String, Result<Module, Err>>,
}

#[async_trait]
impl<'a> WasmModuleCache for WasmModuleMemCache<'a> {
    async fn get(&mut self, key: &str) -> Result<Module, Err> {
        if !self.map.contains_key(key) {
            let wasm_bytes = self.source.get(key).await?;
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
