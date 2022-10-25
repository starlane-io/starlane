#![allow(warnings)]
pub mod err;

#[macro_use]
extern crate lazy_static;


use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Arc, Mutex, RwLock};
use threadpool::ThreadPool;
use tokio::sync::mpsc;
use wasmer::{Store, Module, Instance, Value, imports, WasmerEnv, Array, WasmPtr};
use cosmic_space::artifact::ArtRef;
use cosmic_space::artifact::asynch::ArtifactApi;
use cosmic_space::config::mechtron::MechtronConfig;
use cosmic_space::err::SpaceErr;
use cosmic_space::loc::Point;
use cosmic_space::particle::{Details, Property};
use cosmic_space::substance::Bin;
use cosmic_space::wave::UltraWave;
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


  pub fn wave_to_guest(&self, wave: UltraWave) -> Result<i32,DefaultHostErr> {
        let wave: Vec<u8> = bincode::serialize(&wave)?;
        Ok(self.write_buffer(&wave)?)
    }


    pub fn route(&self, wave: UltraWave) -> Result<Option<UltraWave>, DefaultHostErr> {
      let wave = self.wave_to_guest(wave)?;

        let reflect = self
            .instance
            .exports
            .get_native_function::<i32, i32>("mechtron_frame_to_guest")
            .unwrap()
            .call(wave)?;

        if reflect == 0 {
            Ok(None)
        } else {
            let reflect = self.consume_buffer(reflect)?;
            let reflect = reflect.as_slice();
            let reflect: UltraWave = bincode::deserialize(reflect)?;
            Ok(Some(reflect))
        }
    }


  pub fn write_string<S: ToString>(&self, string: S) -> Result<i32, DefaultHostErr> {
    let string = string.to_string();
    let string = string.as_bytes();
    let memory = self.instance.exports.get_memory("memory")?;
    let buffer_id = self.alloc_buffer(string.len() as _)?;
    let buffer_ptr = self.get_buffer_ptr(buffer_id)?;
    let values = buffer_ptr.deref(memory, 0, string.len() as u32).unwrap();
    for i in 0..string.len() {
      values[i].set(string[i]);
    }

    Ok(buffer_id)
  }

  pub fn write_buffer(&self, bytes: &Vec<u8>) -> Result<i32, DefaultHostErr> {
    let memory = self.instance.exports.get_memory("memory")?;
    let buffer_id = self.alloc_buffer(bytes.len() as _)?;
    let buffer_ptr = self.get_buffer_ptr(buffer_id)?;
    let values = buffer_ptr.deref(memory, 0, bytes.len() as u32).unwrap();
    for i in 0..bytes.len() {
      values[i].set(bytes[i]);
    }

    Ok(buffer_id)
  }

  fn alloc_buffer(&self, len: i32) -> Result<i32, DefaultHostErr> {
    let buffer_id = self
        .instance
        .exports
        .get_native_function::<i32, i32>("mechtron_guest_alloc_buffer")
        .unwrap()
        .call(len.clone())?;
    Ok(buffer_id)
  }

  fn get_buffer_ptr(&self, buffer_id: i32) -> Result<WasmPtr<u8, Array>, DefaultHostErr> {
    Ok(self
        .instance
        .exports
        .get_native_function::<i32, WasmPtr<u8, Array>>("mechtron_guest_get_buffer_ptr")
        .unwrap()
        .call(buffer_id)?)
  }

  pub fn read_buffer(&self, buffer_id: i32) -> Result<Vec<u8>, DefaultHostErr> {
    let ptr = self
        .instance
        .exports
        .get_native_function::<i32, WasmPtr<u8, Array>>("mechtron_guest_get_buffer_ptr")
        .unwrap()
        .call(buffer_id)?;
    let len = self
        .instance
        .exports
        .get_native_function::<i32, i32>("mechtron_guest_get_buffer_len")
        .unwrap()
        .call(buffer_id)?;
    let memory = self.instance.exports.get_memory("memory")?;
    let values = ptr.deref(memory, 0, len as u32).unwrap();
    let mut rtn = vec![];
    for i in 0..values.len() {
      rtn.push(values[i].get())
    }

    Ok(rtn)
  }

  pub fn read_string(&self, buffer_id: i32) -> Result<String, DefaultHostErr> {
    let raw = self.read_buffer(buffer_id)?;
    let rtn = String::from_utf8(raw)?;

    Ok(rtn)
  }

  pub fn consume_string(&self, buffer_id: i32) -> Result<String, DefaultHostErr> {
    let raw = self.read_buffer(buffer_id)?;
    let rtn = String::from_utf8(raw)?;
    self.mechtron_guest_dealloc_buffer(buffer_id)?;
    Ok(rtn)
  }

  pub fn consume_buffer(&self, buffer_id: i32) -> Result<Vec<u8>, DefaultHostErr> {
    let raw = self.read_buffer(buffer_id)?;
    self.mechtron_guest_dealloc_buffer(buffer_id)?;
    Ok(raw)
  }

  fn mechtron_guest_dealloc_buffer(&self, buffer_id: i32) -> Result<(), DefaultHostErr> {
    self.instance
        .exports
        .get_native_function::<i32, ()>("mechtron_guest_dealloc_buffer")?
        .call(buffer_id.clone())?;
    Ok(())
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
