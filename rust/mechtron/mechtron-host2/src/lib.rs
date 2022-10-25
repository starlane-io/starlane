#![allow(warnings)]
pub mod err;

#[macro_use]
extern crate lazy_static;


use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Arc, Mutex, RwLock};
use threadpool::ThreadPool;
use tokio::sync::mpsc;
use wasmer::{Store, Module, Instance, Value, imports, WasmerEnv};
use cosmic_space::artifact::ArtRef;
use cosmic_space::artifact::asynch::ArtifactApi;
use cosmic_space::config::mechtron::MechtronConfig;
use cosmic_space::err::SpaceErr;
use cosmic_space::loc::Point;
use cosmic_space::particle::{Details, Property};
use cosmic_space::substance::Bin;
use crate::err::{DefaultHostErr, HostErr};

pub enum MechtronHostsCall {
  Create { details: Details, rtn: oneshot::Sender<Result<(),SpaceErr>> }
}

pub struct MechtronHostsRunner {
  store: Store,
  threads: ThreadPool,
  rx: mpsc::Receiver<MechtronHostsCall>,
  artifacts: ArtifactApi,
  hosts: HashMap<Point, WasmHost>,
  mechtron_to_host: HashMap<Point,Point>
}

impl MechtronHostsRunner {

  pub fn new(artifacts: ArtifactApi) -> mpsc::Sender<MechtronHostsCall> {
    let (tx,rx) = mpsc::channel(1024);
    let runner = Self {
      store: Store::default(),
      threads: ThreadPool::new(5),
      rx,
      artifacts,
      hosts: Default::default(),
      mechtron_to_host: Default::default()
    };
    tokio::spawn( async move {
      runner.start();
    });
    tx
  }

  async fn start( mut self )  {
    while let Some(call) = self.rx.recv().await {
      match call {
        MechtronHostsCall::Create { details, rtn } => {
          rtn.send(self.create(details).await).unwrap_or_default();
        }
      }
    }
  }

  async fn create(&mut self, details: Details ) -> Result<(),SpaceErr> {
    let config = details.properties.get(&"config".to_string()).ok_or(SpaceErr::bad_request("expected 'config' property to be set"))?;
    let config = self.artifacts.mechtron(&Point::from_str(config.value.as_str())?).await?;
    let host = if let Some(host) = self.hosts.get(&config.wasm ) {
      host
    } else {
      let wasm = self.artifacts.wasm(&config.wasm).await?;
      let host = WasmHost::new(&mut self.store, wasm ).map_err(|e|e.to_space_err())?;
      self.hosts.insert( config.wasm.clone(), host );
      self.hosts.get(&config.wasm).unwrap()
    };

    Ok(())
  }
}


pub enum WasmHostCall {

}

pub struct WasmHost {
  instance: Instance,
}

impl WasmHost {

  pub fn new( store: &mut Store, wasm: ArtRef<Bin>) -> Result<WasmHost,DefaultHostErr> {
    let module = Module::new(store, wasm.as_slice())?;
    let import_object = imports! {};
    let instance = Instance::new(&module,  &import_object)?;

    Ok(Self {
      instance
    })
  }
}



#[derive(WasmerEnv, Clone)]
struct Env
{
  host: Arc<WasmHost>
}

impl Env{
  pub fn new(host: Arc<WasmHost>) -> Self {
    Self {
      host
    }
  }
}




#[cfg(test)]
pub mod test{
  #[test]
  pub fn test() {

  }
}
