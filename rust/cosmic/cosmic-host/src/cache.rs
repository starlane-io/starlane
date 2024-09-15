use std::cell::Cell;
use crate::err::Err;
use crate::src::Source;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process;
use bytes::{Bytes, BytesMut};
use wasmer::{Module, Store};
use wasmer_compiler_singlepass::Singlepass;
#[async_trait]
pub trait WasmModuleCache {
    async fn get(&mut self, key: &str, store: & Store) -> Result<Module, Err>;
}

pub struct WasmModuleMemCache {
    source: Box<dyn Source>,
    map: HashMap<String, Result<Module, Err>>,
    ser: Option<SerializedCache>
}
impl WasmModuleMemCache {
    pub fn new( source: Box<dyn Source> ) -> Self {
        Self {
            source,
            map: Default::default(),
            ser: Option::None
        }
    }

    pub fn new_with_ser( source: Box<dyn Source>, ser_path: PathBuf ) -> Self {
        let ser = SerializedCache::new(ser_path);
        Self {
            source,
            map: Default::default(),
            ser: Some(ser)

        }
    }

}

#[async_trait]
impl WasmModuleCache for WasmModuleMemCache {
    async fn get(&mut self, key: &str, store: & Store) -> Result<Module, Err> {

println!("Getting from STORE {}", key);
        async fn load( source: &Box<dyn Source>, key: &str, store: & Store ) -> Result<Module, Err> {
println!("loading {}", key);
            let wasm_bytes = source.get(key).await?;
            let module = Module::new(store, wasm_bytes).map_err(|e| e.into());
            module
        }

        if !self.map.contains_key(key) {
println!("not loaded: {}", key);
            if let Some(ser) = &self.ser {
                println!("check if ser...: {}", key);
                if let Option::Some(Result::Ok(module)) =  ser.get( &key.to_string(), store ).await
                {
                    self.map.insert(key.to_string(), Result::Ok(module));
                }
                else
                {
                    let rtn = load(&self.source, key, store).await;

                    println!("loaded from wasm...: {}", key);
                    if let Result::Ok(module) = &rtn {

                        println!("storing ser...: {}", key);
                        match ser.store( &key.to_string(), module,  ).await {
                            Ok(_) => {
                                println!("saved module: {}",key);
                            }
                            Err(err) => {

                                eprintln!("error ser module: {}", err.to_string() );
                            }
                        }

                    }
                    self.map.insert(key.to_string(), rtn);
                }
            } else {
                self.map.insert(key.to_string(), load(&self.source, key, store).await);
            }
        }

        match self.map.get(key).unwrap() {
            Ok(module) => Result::Ok(module.clone()),
            Err(err) => Result::Err(err.clone()),
        }
    }
}

impl WasmModuleMemCache {}


pub struct SerializedCache {
    path: PathBuf
}

impl SerializedCache {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path
        }
    }

    pub async fn get(& self, name: &str, store: & Store) -> Option<Result<Module,Err>> {
        let file = self.path.join(Path::new(format!("{}.ser",name).as_str()));
        if ! file.exists() {
            return Option::None;
        }
        let result = unsafe {
            Module::deserialize_from_file(&store, file).map_err(|e| e.to_string())
        };

        Some(result.map_err(|e| e.into()))
    }

    pub async fn store( & self, name: &str , module: & Module  ) -> Result<(),Err> {
        let file = self.path.join(Path::new(format!("{}.ser",name).as_str()));
        module.serialize_to_file(file).map_err(|e| e.into())
    }
}