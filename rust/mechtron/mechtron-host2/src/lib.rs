#![allow(warnings)]
pub mod err;

#[macro_use]
extern crate lazy_static;

use crate::err::{DefaultHostErr, HostErr};
use cosmic_space::artifact::asynch::{ArtifactApi, ReadArtifactFetcher};
use cosmic_space::artifact::ArtRef;
use cosmic_space::config::mechtron::MechtronConfig;
use cosmic_space::err::SpaceErr;
use cosmic_space::loc::{Point, ToSurface};
use cosmic_space::particle::{Details, Property};
use cosmic_space::substance::Bin;
use cosmic_space::wave::DirectedWave;
use cosmic_space::wave::{DirectedKind, UltraWave, WaveKind};

use wasmer::Function;
use wasmer_compiler_singlepass::Singlepass;

use cosmic_space::hyper::{HostCmd, HyperSubstance};
use cosmic_space::log::{LogSource, PointLogger, RootLogger, StdOutAppender};
use cosmic_space::substance::Substance;
use cosmic_space::wasm::Timestamp;
use cosmic_space::wave::core::hyp::HypMethod;
use cosmic_space::wave::{Agent, DirectedProto};
use cosmic_space::{loc, VERSION};

use cosmic_space::wave::core::cmd::CmdMethod;
use cosmic_space::wave::core::Method;
use cosmic_space::wave::exchange::asynch::ProtoTransmitter;
use cosmic_space::wave::exchange::asynch::ProtoTransmitterBuilder;
use cosmic_space::wave::exchange::SetStrategy;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Arc, Mutex, RwLock};
use std::{sync, thread};
use threadpool::ThreadPool;
use tokio::runtime::Handle;
use tokio::sync::mpsc;
use wasmer::{imports, Array, Instance, Module, Store, Value, WasmPtr, WasmerEnv};

pub enum MechtronHostsCall {
    Create {
        details: Details,
        rtn: oneshot::Sender<Result<(), SpaceErr>>,
    },
}

pub struct MechtronHostsRunner {
    store: Store,
    artifacts: ArtifactApi,
    hosts: HashMap<Point, WasmHostApi>,
    mechtron_to_host: HashMap<Point, Point>,
    transmitter: ProtoTransmitterBuilder,
}

impl MechtronHostsRunner {
    pub fn new(
        artifacts: ArtifactApi,
        transmitter: ProtoTransmitterBuilder,
    ) -> mpsc::Sender<MechtronHostsCall> {
        let (tx, rx) = mpsc::channel(1024);
        let runner = Self {
            store: Store::default(),
            //rx,
            artifacts,
            hosts: Default::default(),
            mechtron_to_host: Default::default(),
            transmitter,
        };
        tokio::spawn(async move {
            runner.start();
        });
        tx
    }

    async fn start(mut self) {
        /*        while let Some(call) = self.rx.recv().await {
                   match call {
                       MechtronHostsCall::Create { details, rtn } => {
                           rtn.send(self.create(details,self.transmitter.clone()).await).unwrap_or_default();
                       }
                   }
               }

        */
    }

    async fn create(
        &mut self,
        details: Details,
        mut transmitter: ProtoTransmitterBuilder,
    ) -> Result<(), SpaceErr> {
        transmitter.via = SetStrategy::Override(details.stub.point.to_surface());
        let transmitter = transmitter.build();
        let config = details
            .properties
            .get(&"config".to_string())
            .ok_or(SpaceErr::bad_request(
                "expected 'config' property to be set",
            ))?;
        let config = self
            .artifacts
            .mechtron(&Point::from_str(config.value.as_str())?)
            .await?;
        let host = if let Some(host) = self.hosts.get(&config.wasm) {
            host
        } else {
            let wasm = self.artifacts.wasm(&config.wasm).await?;
            let host =
                WasmHost::new(&mut self.store, wasm, transmitter).map_err(|e| e.to_space_err())?;
            self.hosts.insert(config.wasm.clone(), host);
            self.hosts.get(&config.wasm).unwrap()
        };

        Ok(())
    }
}

pub struct WasmHostSkel {
    pool: Arc<ThreadPool>,
}

#[derive(Debug)]
pub enum WasmHostCall {
    WriteString {
        string: String,
        rtn: tokio::sync::oneshot::Sender<Result<i32, DefaultHostErr>>,
    },
    WriteBuffer {
        buffer: Vec<u8>,
        rtn: tokio::sync::oneshot::Sender<Result<i32, DefaultHostErr>>,
    },
    WaveToGuest {
        wave: UltraWave,
        rtn: tokio::sync::oneshot::Sender<Result<Option<UltraWave>, DefaultHostErr>>,
    },
    WaveToHost {
        wave: UltraWave,
        rtn: tokio::sync::oneshot::Sender<Result<Option<UltraWave>, DefaultHostErr>>,
    },
    ConsumeString {
        buffer_id: i32,
        rtn: tokio::sync::oneshot::Sender<Result<String, DefaultHostErr>>,
    },
    ConsumeBuffer {
        buffer_id: i32,
        rtn: tokio::sync::oneshot::Sender<Result<Vec<u8>, DefaultHostErr>>,
    },
}

#[derive(WasmerEnv, Clone)]
pub struct WasmHostApi {
    tx: Arc<Mutex<std::sync::mpsc::Sender<WasmHostCall>>>,
}

impl WasmHostApi {
    pub fn new(tx: std::sync::mpsc::Sender<WasmHostCall>) -> Self {
        let tx = Arc::new(Mutex::new(tx));
        Self { tx }
    }

    pub fn write_string<S: ToString>(&self, string: S) -> Result<i32, DefaultHostErr> {
        let (rtn, mut rtn_rx) = tokio::sync::oneshot::channel();
        self.tx
            .lock()?
            .send(WasmHostCall::WriteString {
                string: string.to_string(),
                rtn,
            })
            .unwrap();
        rtn_rx.blocking_recv()?
    }

    pub fn consume_string(&self, buffer_id: i32) -> Result<String, DefaultHostErr> {
        let (rtn, mut rtn_rx) = tokio::sync::oneshot::channel();
        self.tx
            .lock()?
            .send(WasmHostCall::ConsumeString { buffer_id, rtn })
            .unwrap();
        rtn_rx.blocking_recv()?
    }

    pub fn consume_buffer(&self, buffer_id: i32) -> Result<Vec<u8>, DefaultHostErr> {
        let (rtn, mut rtn_rx) = tokio::sync::oneshot::channel();
        self.tx
            .lock()?
            .send(WasmHostCall::ConsumeBuffer { buffer_id, rtn })
            .unwrap();
        rtn_rx.blocking_recv()?
    }

    pub fn mechtron_frame_to_host(
        &self,
        buffer_id: i32,
    ) -> Result<Option<UltraWave>, DefaultHostErr> {
        let (rtn, mut rtn_rx) = tokio::sync::oneshot::channel();
        let wave = self.consume_buffer(buffer_id)?;
        let wave: UltraWave = bincode::deserialize(wave.as_slice())?;
        self.tx
            .lock()?
            .send(WasmHostCall::WaveToHost { wave, rtn })
            .unwrap();
        rtn_rx.blocking_recv()?
    }
}

pub struct WasmHost {
    instance: Instance,
    pub transmitter: ProtoTransmitter,
    handle: Handle,
    rx: std::sync::mpsc::Receiver<WasmHostCall>,
}

impl WasmHost {
    pub fn new(
        store: &mut Store,
        wasm: ArtRef<Bin>,
        transmitter: ProtoTransmitter,
    ) -> Result<WasmHostApi, DefaultHostErr> {
        let module = Module::new(store, wasm.as_slice())?;

        let (tx, rx) = std::sync::mpsc::channel();
        let host = WasmHostApi::new(tx);

        let handle = Handle::current();

        let imports = imports! {

            "env"=>{
             "mechtron_timestamp"=>Function::new_native_with_env(module.store(),host.clone(),|env:&WasmHostApi| {
                    chrono::Utc::now().timestamp_millis()
            }),

        "mechtron_uuid"=>Function::new_native_with_env(module.store(),host.clone(),|env:&WasmHostApi | -> i32 {
              env.write_string(uuid::Uuid::new_v4().to_string().as_str()).unwrap()
            }),

        "mechtron_host_panic"=>Function::new_native_with_env(module.store(),host.clone(),|env:&WasmHostApi,buffer_id:i32| {
              let panic_message = env.consume_string(buffer_id).unwrap();
               println!("WASM PANIC: {}",panic_message);
          }),


        "mechtron_frame_to_host"=>Function::new_native_with_env(module.store(),host.clone(),|env:&WasmHostApi,buffer_id:i32| -> i32 {
                    match env.mechtron_frame_to_host(buffer_id).unwrap() {
                        Some( wave ) => {
                            0
                        }
                        None => 0
                    }
                    /*
                    let (tx,mut rx) = oneshot::channel();


                    env.handle.spawn( async move {

                        if wave.is_directed() {
                            let wave = wave.to_directed().unwrap();
                        match wave {
                            DirectedWave::Ping(ping) => {
                                let proto: DirectedProto = ping.into();
                                let pong = transmitter.ping(proto);
                                let rtn = host.wave_to_guest(pong.to_ultra()).unwrap();
                                tx.send(rtn);
                                return;
                            }
                            DirectedWave::Ripple(ripple) => {
                                    unimplemented!()
                            }
                            DirectedWave::Signal(signal) => {
                                transmitter.route( signal.to_ultra() ).await;
                            }
                        }
                    } else {
                        transmitter.route(wave).await;
                    }

                    tx.send(0i32);

                 });
                     */
            }),

        } };
        let instance = Instance::new(&module, &imports)?;

        WasmHost {
            instance,
            transmitter,
            handle,
            rx,
        }
        .start();

        Ok(host)
    }

    pub fn wave_to_host(&self, wave: UltraWave) -> Result<Option<UltraWave>, DefaultHostErr> {
        let transmitter = self.transmitter.clone();
        let (tx, mut rx): (
            oneshot::Sender<Result<Option<UltraWave>, DefaultHostErr>>,
            oneshot::Receiver<Result<Option<UltraWave>, DefaultHostErr>>,
        ) = oneshot::channel();
        self.handle.spawn(async move {
            if wave.is_directed() {
                let wave = wave.to_directed().unwrap();
                match wave.directed_kind() {
                    DirectedKind::Ping => {
                        let wave: DirectedProto = wave.into();
                        let pong = transmitter.ping(wave).await.unwrap();
                        tx.send(Ok(Some(pong.to_ultra()))).unwrap_or_default();
                    }
                    DirectedKind::Ripple => {
                        unimplemented!()
                    }
                    DirectedKind::Signal => {
                        let wave: DirectedProto = wave.into();
                        transmitter.signal(wave).await.unwrap_or_default();
                        tx.send(Ok(None));
                    }
                }
            } else {
                transmitter.route(wave).await;
                tx.send(Ok(None));
            }
        });

        rx.recv()?
    }

    pub fn wave_to_guest(&self, wave: UltraWave) -> Result<i32, DefaultHostErr> {
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

    fn start(mut self) {
        thread::spawn(move || {
            while let Ok(call) = self.rx.recv() {
                match call {
                    WasmHostCall::WriteString { string, rtn } => {
                        rtn.send(self.write_string(string));
                    }
                    WasmHostCall::WriteBuffer { buffer, rtn } => {
                        rtn.send(self.write_buffer(&buffer));
                    }

                    WasmHostCall::ConsumeString { buffer_id, rtn } => {
                        rtn.send(self.consume_string(buffer_id));
                    }
                    WasmHostCall::ConsumeBuffer { buffer_id, rtn } => {
                        rtn.send(self.consume_buffer(buffer_id));
                    }
                    WasmHostCall::WaveToGuest { wave, rtn } => {
                        rtn.send(self.route(wave));
                    }
                    WasmHostCall::WaveToHost { wave, rtn } => {
                        rtn.send(self.wave_to_host(wave));
                    }
                }
            }
        });
    }
}

#[cfg(test)]
pub mod test {
    #[test]
    pub fn test() {}
}
