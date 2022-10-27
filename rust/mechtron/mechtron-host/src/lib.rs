#![allow(warnings)]
pub mod err;

#[macro_use]
extern crate lazy_static;

use crate::err::{DefaultHostErr, HostErr};
use cosmic_space::artifact::asynch::{ArtifactApi, ReadArtifactFetcher};
use cosmic_space::artifact::ArtRef;
use cosmic_space::config::mechtron::MechtronConfig;
use cosmic_space::err::SpaceErr;
use cosmic_space::loc::{Layer, Point, ToSurface};
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

#[derive(Clone)]
pub struct HostsApi {
    tx: tokio::sync::mpsc::Sender<HostsCall>
}

impl HostsApi {
    pub async fn get(&self, wasm: &Point ) -> Result<WasmHostApi,SpaceErr> {
       let (rtn,mut rtn_rx)  = tokio::sync::oneshot::channel();
       self.tx.send(HostsCall::Get{ wasm: wasm.clone(), rtn}).await?;
        rtn_rx.await?
    }

    pub async fn create( &self, details: Details, wasm: Point) -> Result<WasmHostApi,SpaceErr> {
       let (rtn,mut rtn_rx)  = tokio::sync::oneshot::channel();
       self.tx.send(HostsCall::Create{ details, wasm, rtn}).await?;
        rtn_rx.await?
    }
}

pub enum HostsCall {
    Get{
        wasm: Point,
        rtn: tokio::sync::oneshot::Sender<Result<WasmHostApi, SpaceErr>>,
    },
    Create {
        details: Details,
        wasm: Point,
        rtn: tokio::sync::oneshot::Sender<Result<WasmHostApi, SpaceErr>>,
    },
}

pub struct HostsRunner {
    store: Store,
    artifacts: ArtifactApi,
    wasm_to_host: HashMap<Point, WasmHostApi>,
    point_to_host : HashMap<Point, WasmHostApi>,
    mechtron_to_host: HashMap<Point, Point>,
    transmitter: ProtoTransmitterBuilder,
    logger: RootLogger,
    rx: tokio::sync::mpsc::Receiver<HostsCall>
}

impl HostsRunner {
    pub fn new(
        artifacts: ArtifactApi,
        transmitter: ProtoTransmitterBuilder,
        logger: RootLogger,
    ) -> HostsApi {
        let (tx, rx) = mpsc::channel(1024);
        let runner = Self {
            rx,
            store: Store::default(),
            artifacts,
            wasm_to_host: Default::default(),
            point_to_host: Default::default(),
            mechtron_to_host: Default::default(),
            transmitter,
            logger,
        };
        tokio::spawn(async move {
            runner.start().await;
        });
        HostsApi {
            tx
        }
    }

    async fn start(mut self) {
                while let Some(call) = self.rx.recv().await {

                   match call {
                       HostsCall::Create { details,wasm, rtn } => {
                           rtn.send(self.create_host(details, wasm, self.transmitter.clone()).await).unwrap_or_default();
                       }
                       HostsCall::Get { wasm, rtn } => {
                          rtn.send(self.wasm_to_host.get( &wasm ).cloned().ok_or(format!("could not get host: {}", wasm.to_string()).into()) );

                       }
                   }
               }

    }

    async fn create_host(
        &mut self,
        details: Details,
        wasm: Point,
        mut transmitter: ProtoTransmitterBuilder,
    ) -> Result<WasmHostApi, SpaceErr> {
        transmitter.via = SetStrategy::Override(details.stub.point.to_surface());
        let transmitter = transmitter.build();

        let logger = self.logger.point(details.stub.point.clone());
        let bin = self.artifacts.wasm(&wasm).await?;
        let host = WasmHostRunner::new(details.clone(), &mut self.store, bin, transmitter, logger)
                .map_err(|e| e.to_space_err())?;
         self.wasm_to_host.insert(wasm.clone(), host.clone());
         self.point_to_host.insert(details.stub.point,host.clone());

        host.init().await;

        Ok(host)
    }

    pub fn get(&self, point: &Point) -> Result<WasmHostApi,SpaceErr> {
        self.point_to_host.get(point).cloned().ok_or(format!("cannot find host: {}",point.to_string()).into())
    }
}

pub struct WasmHostSkel {
    pool: Arc<ThreadPool>,
}

#[derive(Debug)]
pub enum WasmHostCall {
    Init(tokio::sync::oneshot::Sender<Result<(), DefaultHostErr>>),
    HostCmd{cmd: HostCmd, rtn: tokio::sync::oneshot::Sender<Result<(), DefaultHostErr>>},
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
        rtn: tokio::sync::oneshot::Sender<Result<i32, DefaultHostErr>>,
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

#[derive(WasmerEnv,Clone)]
pub struct WasmHostApi {
    tx: Arc<Mutex<std::sync::mpsc::Sender<WasmHostCall>>>,
}

impl WasmHostApi {
    pub fn new( tx: std::sync::mpsc::Sender<WasmHostCall>) -> Self {
        let tx = Arc::new(Mutex::new(tx));
        Self { tx }
    }

    pub async fn init(&self) -> Result<(), DefaultHostErr> {
        let (rtn, mut rtn_rx) = tokio::sync::oneshot::channel();
        self.tx.lock()?.send(WasmHostCall::Init(rtn)).unwrap();
        rtn_rx.await?
    }

    pub async fn create_mechtron( &self, cmd: HostCmd ) -> Result<(),DefaultHostErr>{
        let (rtn, mut rtn_rx) = tokio::sync::oneshot::channel();
        self.tx.lock()?.send(WasmHostCall::HostCmd{cmd,rtn}).unwrap();
        rtn_rx.await?
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

    pub fn write_buffer(&self, buffer: Vec<u8>) -> Result<i32, DefaultHostErr> {
        let (rtn, mut rtn_rx) = tokio::sync::oneshot::channel();
        self.tx
            .lock()?
            .send(WasmHostCall::WriteBuffer { buffer, rtn })
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

    pub fn wave_to_host(&self, buffer_id: i32) -> Result<Option<UltraWave>, DefaultHostErr> {
        let (rtn, mut rtn_rx) = tokio::sync::oneshot::channel();
        let wave = self.consume_buffer(buffer_id)?;
        let wave: UltraWave = bincode::deserialize(wave.as_slice())?;


        self.tx
            .lock()?
            .send(WasmHostCall::WaveToHost { wave, rtn })
            .unwrap();
        rtn_rx.blocking_recv()?
    }

    pub fn wave_to_guest(&self, wave: UltraWave) -> Result<i32, DefaultHostErr> {
        let (rtn, mut rtn_rx) = tokio::sync::oneshot::channel();
        self.tx
            .lock()?
            .send(WasmHostCall::WaveToGuest { wave, rtn })
            .unwrap();
        rtn_rx.blocking_recv()?
    }

    pub fn transmit_to_guest(&self, wave: UltraWave) -> Result<Option<UltraWave>,DefaultHostErr> {
       let wave_id = self.wave_to_guest(wave)?;
        if wave_id <= 0 {
            Ok(None)
        } else {
            let buffer = self.consume_buffer(wave_id)?;
            let wave: UltraWave = bincode::deserialize(buffer.as_slice())?;
            Ok(Some(wave))
        }
    }

    pub fn host_mechtron(&self, cmd: HostCmd) {
    }
}

pub struct WasmHostRunner {
    pub rx: std::sync::mpsc::Receiver<WasmHostCall>,
    pub host: WasmHost,
}

impl WasmHostRunner {
    pub fn new(
        details: Details,
        store: &mut Store,
        wasm: ArtRef<Bin>,
        transmitter: ProtoTransmitter,
        logger: PointLogger,
    ) -> Result<WasmHostApi, DefaultHostErr> {
        let module = Module::new(store, wasm.as_slice())?;

        let (tx, rx) = std::sync::mpsc::channel();

        let handle = Handle::current();

        let api = WasmHostApi::new(tx);

        let imports = imports! {

        "env"=>{
             "mechtron_timestamp"=>Function::new_native_with_env(module.store(),api.clone(),|env:&WasmHostApi| {
                    chrono::Utc::now().timestamp_millis()
            }),

        "mechtron_uuid"=>Function::new_native_with_env(module.store(),api.clone(),|env:&WasmHostApi | -> i32 {
              env.write_string(uuid::Uuid::new_v4().to_string().as_str()).unwrap()
            }),

        "mechtron_host_panic"=>Function::new_native_with_env(module.store(),api.clone(),|env:&WasmHostApi,buffer_id:i32| {
              let panic_message = env.consume_string(buffer_id).unwrap();
               println!("WASM PANIC: {}",panic_message);
          }),


        "mechtron_frame_to_host"=>Function::new_native_with_env(module.store(),api.clone(),|env:&WasmHostApi,buffer_id:i32| -> i32 {
                    match env.wave_to_host(buffer_id).unwrap() {
                        Some( wave ) => {
                           env.wave_to_guest(wave).unwrap()
                        }
                        None => 0
                    }
            }),

        } };
        let instance = Instance::new(&module, &imports)?;

        let host = WasmHost {
            details,
            instance,
            transmitter,
            handle,
            logger,
        };

        Self { host, rx }.start();

        Ok(api)
    }

    pub fn start(mut self) {
        thread::spawn(move || {
            let pool = ThreadPool::new(10);
            while let Ok(call) = self.rx.recv() {
                let host = self.host.clone();
                pool.execute(move || match call {
                    WasmHostCall::Init(rtn) => {
                        rtn.send(host.init());
                    }
                    WasmHostCall::WriteString { string, rtn } => {
                        rtn.send(host.write_string(string));
                    }
                    WasmHostCall::WriteBuffer { buffer, rtn } => {
                        rtn.send(host.write_buffer(&buffer));
                    }
                    WasmHostCall::ConsumeString { buffer_id, rtn } => {
                        rtn.send(host.consume_string(buffer_id));
                    }
                    WasmHostCall::ConsumeBuffer { buffer_id, rtn } => {
                        rtn.send(host.consume_buffer(buffer_id));
                    }
                    WasmHostCall::WaveToGuest { wave, rtn } => {
                        rtn.send(host.route(wave));
                    }
                    WasmHostCall::WaveToHost { wave, rtn } => {
                        rtn.send(host.wave_to_host(wave));
                    }
                    WasmHostCall::HostCmd { cmd, rtn } => {
                        rtn.send(host.create_mechtron(cmd));
                    }
                });
            }
        });
    }
}

#[derive(Clone)]
pub struct WasmHost {
    details: Details,
    instance: Instance,
    pub transmitter: ProtoTransmitter,
    handle: Handle,
    logger: PointLogger,
}

impl WasmHost {
    pub fn new(
        details: Details,
        store: &mut Store,
        wasm: ArtRef<Bin>,
        transmitter: ProtoTransmitter,
        logger: PointLogger,
    ) -> Result<Self, DefaultHostErr> {
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
                    match env.wave_to_host(buffer_id).unwrap() {
                        Some( wave ) => {
                           env.wave_to_guest(wave).unwrap()
                        }
                        None => 0
                    }
            }),

        } };
        let instance = Instance::new(&module, &imports)?;

        Ok(WasmHost {
            details,
            instance,
            transmitter,
            handle,
            logger,
        })
    }

    pub fn init(&self) -> Result<(), DefaultHostErr> {
        let mut pass = true;
        match self.instance.exports.get_memory("memory") {
            Ok(_) => {
                self.logger.info("verified: memory");
            }
            Err(_) => {
                self.logger.info( "failed: memory. could not access wasm memory. (expecting the memory module named 'memory')");
                pass = false
            }
        }

        match self
            .instance
            .exports
            .get_native_function::<i32, i32>("mechtron_guest_alloc_buffer")
        {
            Ok(_) => {
                self.logger
                    .info("verified: mechtron_guest_alloc_buffer( i32 ) -> i32");
            }
            Err(_) => {
                self.logger
                    .info("failed: mechtron_guest_alloc_buffer( i32 ) -> i32");
                pass = false
            }
        }

        match self
            .instance
            .exports
            .get_native_function::<i32, WasmPtr<u8, Array>>("mechtron_guest_get_buffer_ptr")
        {
            Ok(_) => {
                self.logger
                    .info("verified: mechtron_guest_get_buffer_ptr( i32 ) -> *const u8");
            }
            Err(_) => {
                self.logger
                    .info("failed: mechtron_guest_get_buffer_ptr( i32 ) -> *const u8");
                pass = false
            }
        }

        match self
            .instance
            .exports
            .get_native_function::<i32, i32>("mechtron_guest_get_buffer_len")
        {
            Ok(_) => {
                self.logger
                    .info("verified: mechtron_guest_get_buffer_len( i32 ) -> i32");
            }
            Err(_) => {
                self.logger
                    .info("failed: mechtron_guest_get_buffer_len( i32 ) -> i32");
                pass = false
            }
        }
        match self
            .instance
            .exports
            .get_native_function::<i32, ()>("mechtron_guest_dealloc_buffer")
        {
            Ok(_) => {
                self.logger
                    .info("verified: mechtron_guest_dealloc_buffer( i32 )");
            }
            Err(_) => {
                self.logger
                    .info("failed: mechtron_guest_dealloc_buffer( i32 )");
                pass = false
            }
        }

        match self
            .instance
            .exports
            .get_native_function::<(i32, i32), i32>("mechtron_guest_init")
        {
            Ok(func) => {
                self.logger.info("verified: mechtron_guest_init()");
            }
            Err(_) => {
                self.logger
                    .info("failed: mechtron_guest_init() [NOT REQUIRED]");
            }
        }

        {
            let test = "Test write string";
            match self.write_string(test) {
                Ok(_) => {
                    self.logger.info("passed: write_string()");
                }
                Err(e) => {
                    self.logger
                        .error(format!("failed: write_string() mem {:?}", e).as_str());
                    pass = false;
                }
            };
        }

        match pass {
            true => {

                let version = self.write_string(VERSION.to_string())?;
                let details: Vec<u8> = bincode::serialize(&self.details)?;
                let details = self.write_buffer(&details)?;
                let ok = self
                    .instance
                    .exports
                    .get_native_function::<(i32, i32), i32>("mechtron_guest_init")
                    .unwrap()
                    .call(version, details)?;
                if ok == 0 {
                    Ok(())
                } else {
                    Err(format!("Mechtron init error {} ", ok).into())
                }
            },
            false => Err("init failed".into()),
        }
    }

    pub fn wave_to_host(&self, wave: UltraWave) -> Result<Option<UltraWave>, DefaultHostErr> {
        let transmitter = self.transmitter.clone();
        let (tx, mut rx): (
            oneshot::Sender<Result<Option<UltraWave>, DefaultHostErr>>,
            oneshot::Receiver<Result<Option<UltraWave>, DefaultHostErr>>,
        ) = oneshot::channel();
        let logger = self.logger.clone();
        self.handle.spawn(async move {
            if wave.is_directed() {
                let wave = wave.to_directed().unwrap();

                if let Method::Cmd(CmdMethod::Log) = wave.core().method {
                    if let Substance::Log(log) = wave.core().body.clone() {
                        if wave.to().is_single() {
                            let to = wave.to().clone().to_single().unwrap();
                            if to.point == Point::global_logger() {
                                logger.handle(log);
                                tx.send(Ok(None));
                                return;
                            }
                        }
                    }
                }


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

    pub fn route(&self, wave: UltraWave) -> Result<i32, DefaultHostErr> {
        let wave = self.wave_to_guest(wave)?;

        let reflect = self
            .instance
            .exports
            .get_native_function::<i32, i32>("mechtron_frame_to_guest")
            .unwrap()
            .call(wave)?;

        Ok(reflect)
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

    fn create_mechtron(&self, host_cmd: HostCmd ) -> Result<(),DefaultHostErr> {
        let mut wave = DirectedProto::ping();
        wave.to(self.details.stub.point.to_surface().with_layer(Layer::Core));
        wave.from(self.details.stub.point.to_surface().with_layer(Layer::Host));
        wave.method(HypMethod::Host);
        wave.body(Substance::Hyper(HyperSubstance::Host(host_cmd)));
        let wave = self.logger.result(wave.build())?;
        let wave = wave.to_ultra();
        self.logger.result(self.route(wave))?;
        Ok(())
    }
}

#[cfg(test)]
pub mod test {
    use std::fs;
    use crate::HostsRunner;
    use cosmic_space::loc::Point;
    use cosmic_space::particle::Details;
    use std::str::FromStr;
    use std::sync::Arc;
    use cosmic_space::artifact::asynch::MapFetcher;

    #[tokio::test]
    pub async fn test() {

    }
}
