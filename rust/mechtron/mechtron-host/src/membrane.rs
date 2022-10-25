use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::{Arc, RwLock, Weak};
use tokio::runtime::Handle;

use crate::HostPlatform;
use cosmic_space::err::SpaceErr;
use cosmic_space::loc::Point;
use cosmic_space::log::PointLogger;
use cosmic_space::substance::Substance;
use cosmic_space::wave::core::cmd::CmdMethod;
use cosmic_space::wave::core::Method;
use cosmic_space::wave::{DirectedProto, DirectedWave, UltraWave};
use wasmer::{
    imports, Array, ChainableNamedResolver, Function, ImportObject, Instance, Module,
    NamedResolver, RuntimeError, WasmPtr, WasmerEnv,
};
use cosmic_space::wave::exchange::asynch::ProtoTransmitter;

pub static VERSION: i32 = 1;

pub struct WasmMembrane<P>
where
    P: HostPlatform,
{
    pub instance: Instance,
    init: String,
    name: String,
    platform: P,
    logger: PointLogger,
}

impl<P> WasmMembrane<P>
where
    P: HostPlatform,
{
    pub fn init(&self) -> Result<(), P::Err> {
        let mut pass = true;
        match self.instance.exports.get_memory("memory") {
            Ok(_) => {
                self.log_wasm("host", "verified: memory");
            }
            Err(_) => {
                self.log_wasm("host", "failed: memory. could not access wasm memory. (expecting the memory module named 'memory')");
                pass = false
            }
        }

        match self
            .instance
            .exports
            .get_native_function::<(), i32>("mechtron_guest_version")
        {
            Ok(func) => {
                self.log_wasm("host", "verified: mechtron_guest_version( ) -> i32");
                match func.call() {
                    Ok(version) => {
                        if version == VERSION {
                            self.log_wasm(
                                "host",
                                format!(
                                    "passed: mechtron_guest_version( ) -> i32 [USING VERSION {}]",
                                    version
                                )
                                .as_str(),
                            );
                        } else {
                            self.log_wasm("host", format!("fail : mechtron_guest_version( ) -> i32 [THIS HOST CANNOT WORK WITH VERSION {}]", version).as_str());
                            pass = false;
                        }
                    }
                    Err(error) => {
                        self.log_wasm(
                            "host",
                            "fail : mechtron_guest_version( ) -> i32 [CALL FAILED]",
                        );
                    }
                }
            }
            Err(_) => {
                self.log_wasm("host", "failed: mechtron_guest_version( ) -> i32");
                pass = false
            }
        }

        match self
            .instance
            .exports
            .get_native_function::<i32, i32>("mechtron_guest_alloc_buffer")
        {
            Ok(_) => {
                self.log_wasm(
                    "host",
                    "verified: mechtron_guest_alloc_buffer( i32 ) -> i32",
                );
            }
            Err(_) => {
                self.log_wasm("host", "failed: mechtron_guest_alloc_buffer( i32 ) -> i32");
                pass = false
            }
        }

        match self
            .instance
            .exports
            .get_native_function::<i32, WasmPtr<u8, Array>>("mechtron_guest_get_buffer_ptr")
        {
            Ok(_) => {
                self.log_wasm(
                    "host",
                    "verified: mechtron_guest_get_buffer_ptr( i32 ) -> *const u8",
                );
            }
            Err(_) => {
                self.log_wasm(
                    "host",
                    "failed: mechtron_guest_get_buffer_ptr( i32 ) -> *const u8",
                );
                pass = false
            }
        }

        match self
            .instance
            .exports
            .get_native_function::<i32, i32>("mechtron_guest_get_buffer_len")
        {
            Ok(_) => {
                self.log_wasm(
                    "host",
                    "verified: mechtron_guest_get_buffer_len( i32 ) -> i32",
                );
            }
            Err(_) => {
                self.log_wasm(
                    "host",
                    "failed: mechtron_guest_get_buffer_len( i32 ) -> i32",
                );
                pass = false
            }
        }
        match self
            .instance
            .exports
            .get_native_function::<i32, ()>("mechtron_guest_dealloc_buffer")
        {
            Ok(_) => {
                self.log_wasm("host", "verified: mechtron_guest_dealloc_buffer( i32 )");
            }
            Err(_) => {
                self.log_wasm("host", "failed: mechtron_guest_dealloc_buffer( i32 )");
                pass = false
            }
        }

        match self
            .instance
            .exports
            .get_native_function::<(i32, i32), i32>("mechtron_guest_init")
        {
            Ok(func) => {
                self.log_wasm("host", "verified: mechtron_guest_init()");

                /*                match func.call() {
                                   Ok(_) => {
                                       self.log_wasm("host", "passed: mechtron_guest_init()");
                                   }
                                   Err(error) => {
                                       self.log_wasm(
                                           "host",
                                           format!("failed: mechtron_guest_init() ERROR: {:?}", error).as_str(),
                                       );
                                       pass = false;
                                   }
                               }

                */
            }
            Err(_) => {
                self.log_wasm("host", "failed: mechtron_guest_init() [NOT REQUIRED]");
            }
        }

        {
            let test = "Test write string";
            match self.write_string(test) {
                Ok(_) => {
                    self.log_wasm("host", "passed: write_string()");
                }
                Err(e) => {
                    self.log_wasm(
                        "host",
                        format!("failed: write_string() mem {:?}", e).as_str(),
                    );
                    pass = false;
                }
            };
        }

        match pass {
            true => Ok(()),
            false => Err("init failed".into()),
        }
    }

    pub fn log_wasm(&self, log_type: &str, message: &str) {
        println!("{}({}) : {}", self.name, log_type, message);
    }

    pub fn write_string<S: ToString>(&self, string: S) -> Result<i32, P::Err> {
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

    pub fn write_buffer(&self, bytes: &Vec<u8>) -> Result<i32, P::Err> {
        let memory = self.instance.exports.get_memory("memory")?;
        let buffer_id = self.alloc_buffer(bytes.len() as _)?;
        let buffer_ptr = self.get_buffer_ptr(buffer_id)?;
        let values = buffer_ptr.deref(memory, 0, bytes.len() as u32).unwrap();
        for i in 0..bytes.len() {
            values[i].set(bytes[i]);
        }

        Ok(buffer_id)
    }

    fn alloc_buffer(&self, len: i32) -> Result<i32, P::Err> {
        let buffer_id = self
            .instance
            .exports
            .get_native_function::<i32, i32>("mechtron_guest_alloc_buffer")
            .unwrap()
            .call(len.clone())?;
        Ok(buffer_id)
    }

    fn get_buffer_ptr(&self, buffer_id: i32) -> Result<WasmPtr<u8, Array>, P::Err> {
        Ok(self
            .instance
            .exports
            .get_native_function::<i32, WasmPtr<u8, Array>>("mechtron_guest_get_buffer_ptr")
            .unwrap()
            .call(buffer_id)?)
    }

    pub fn read_buffer(&self, buffer_id: i32) -> Result<Vec<u8>, P::Err> {
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

    pub fn read_string(&self, buffer_id: i32) -> Result<String, P::Err> {
        let raw = self.read_buffer(buffer_id)?;
        let rtn = String::from_utf8(raw)?;

        Ok(rtn)
    }

    pub fn consume_string(&self, buffer_id: i32) -> Result<String, P::Err> {
        let raw = self.read_buffer(buffer_id)?;
        let rtn = String::from_utf8(raw)?;
        self.mechtron_guest_dealloc_buffer(buffer_id)?;
        Ok(rtn)
    }

    pub fn consume_buffer(&self, buffer_id: i32) -> Result<Vec<u8>, P::Err> {
        let raw = self.read_buffer(buffer_id)?;
        self.mechtron_guest_dealloc_buffer(buffer_id)?;
        Ok(raw)
    }

    fn mechtron_guest_dealloc_buffer(&self, buffer_id: i32) -> Result<(), P::Err> {
        self.instance
            .exports
            .get_native_function::<i32, ()>("mechtron_guest_dealloc_buffer")?
            .call(buffer_id.clone())?;
        Ok(())
    }

    pub fn test_panic(&self) -> Result<(), P::Err> {
        self.instance
            .exports
            .get_native_function::<(), ()>("wasm_test_panic")
            .unwrap()
            .call()?;
        Ok(())
    }

    pub fn test_log(&self) -> Result<(), P::Err> {
        let log_message_string = "Hello from Wasm!";
        let log_message_buffer = self.write_string(log_message_string)?;
        self.instance
            .exports
            .get_native_function::<i32, ()>("mechtron_guest_test_log")
            .unwrap()
            .call(log_message_buffer)?;
        Ok(())
    }

    pub async fn test_endless_loop(&self) -> Result<(), P::Err> {
        println!("mem endless loop");
        self.instance
            .exports
            .get_native_function::<(), ()>("mechtron_guest_example_test_endless_loop")
            .unwrap()
            .call()?;
        println!("mem endless loop... done");
        Ok(())
    }
}

#[derive(Clone)]
pub struct WasmBuffer<P>
where
    P: HostPlatform,
{
    ptr: WasmPtr<u8, Array>,
    len: u32,
    phantom: PhantomData<P>,
}

impl<P> WasmBuffer<P>
where
    P: HostPlatform,
{
    pub fn new(ptr: WasmPtr<u8, Array>, len: u32) -> Self {
        WasmBuffer {
            ptr: ptr,
            len: len,
            phantom: Default::default(),
        }
    }
}

pub struct WasmHost<P>
where
    P: HostPlatform,
{
    transmitter: ProtoTransmitter,
    membrane: Option<Weak<WasmMembrane<P>>>,
    logger: PointLogger,
}

impl<P> WasmHost<P>
where
    P: HostPlatform,
{
    fn new(transmitter: ProtoTransmitter,logger: PointLogger) -> Self {
        WasmHost {
            membrane: Option::None,
            logger,
            transmitter
        }
    }
}

#[derive(WasmerEnv, Clone)]
struct Env<P>
where
    P: HostPlatform,
{
    host: Arc<RwLock<WasmHost<P>>>,
}

impl<P> Env<P>
where
    P: HostPlatform,
{
    pub fn unwrap(&self) -> Result<Arc<WasmMembrane<P>>, P::Err> {
        let host = self.host.read();
        if host.is_err() {
            println!("WasmMembrane: could not acquire shell lock");
            return Err("could not acquire shell lock".into());
        }

        let host = host.unwrap();

        let membrane = host.membrane.as_ref();
        if membrane.is_none() {
            println!("WasmMembrane: membrane is not set");
            return Err("membrane is not set".into());
        }
        let membrane = membrane.unwrap().upgrade();

        if membrane.is_none() {
            println!("WasmMembrane: could not upgrade membrane reference");
            return Err("could not upgrade membrane reference".into());
        }
        let membrane = membrane.unwrap();
        let memory = membrane.instance.exports.get_memory("memory");
        if memory.is_err() {
            println!("WasmMembrane: could not access wasm memory");
            return Err("could not access wasm memory".into());
        }
        Ok(membrane)
    }
}

impl<P> WasmMembrane<P>
where
    P: HostPlatform + 'static,
{
    pub fn new(
        module: Arc<Module>,
        name: String,
        platform: P,
        logger: PointLogger,
        transmitter: ProtoTransmitter
    ) -> Result<Arc<Self>, P::Err> {
        Self::new_with_init(
            module,
            name,
            "mechtron_guest_init".to_string(),
            platform,
            logger,
            transmitter
        )
    }

    pub fn new_with_init(
        module: Arc<Module>,
        init: String,
        name: String,
        platform: P,
        logger: PointLogger,
        transmitter: ProtoTransmitter
    ) -> Result<Arc<Self>, P::Err> {
        Self::new_with_init_and_imports(module, init, name, Option::None, platform, logger, transmitter)
    }

    pub fn new_with_init_and_imports(
        module: Arc<Module>,
        init: String,
        name: String,
        ext_imports: Option<ImportObject>,
        platform: P,
        logger: PointLogger,
        transmitter: ProtoTransmitter
    ) -> Result<Arc<Self>, P::Err> {
        let host = Arc::new(RwLock::new(WasmHost::new(transmitter, logger.clone())));

        let imports = imports! {

            "env"=>{
             "mechtron_timestamp"=>Function::new_native_with_env(module.store(),Env{host:host.clone()},|env:&Env<P>| {
                    chrono::Utc::now().timestamp_millis()
            }),

        "mechtron_uuid"=>Function::new_native_with_env(module.store(),Env{host:host.clone()},|env:&Env<P>| -> i32 {
                match env.unwrap()
                {
                   Ok(membrane)=>{
                        let uuid = uuid::Uuid::new_v4().to_string();
                        membrane.write_string(uuid.as_str()).unwrap()
                   },
                   Err(_)=>{
                    -1
                }
                }
            }),


        "mechtron_host_log"=>Function::new_native_with_env(module.store(),Env{host:host.clone()},|env:&Env<P>,buffer:i32| {
                match env.unwrap()
                {
                   Ok(membrane)=>{
                        let message = membrane.consume_string(buffer).unwrap_or("LOG ERROR".to_string());
                        membrane.log_wasm("guest",message.as_str());
                   },
                   Err(_)=>{}
                }
            }),

        "mechtron_host_panic"=>Function::new_native_with_env(module.store(),Env{host:host.clone()},|env:&Env<P>,buffer_id:i32| {
                match env.unwrap()
                {
                   Ok(membrane)=>{
                      let panic_message = membrane.consume_string(buffer_id).unwrap();
                      println!("WASM PANIC: {}",panic_message);
                   },
                   Err(_)=>{
                   println!("error panic");
                   }
                }
            }),


        "mechtron_frame_to_host"=>Function::new_native_with_env(module.store(),Env{host:host.clone()},|env:&Env<P>,buffer_id:i32| -> i32 {

                    let membrane = env.unwrap().unwrap();
                    let wave = membrane.consume_buffer(buffer_id).unwrap();
                    let wave :UltraWave = bincode::deserialize(wave.as_slice()).unwrap();
                    let transmitter = {
                        env.host.read().unwrap().transmitter.clone()
                    };

                    let (tx,mut rx) = oneshot::channel();

                    tokio::spawn( async move {

                        if wave.is_directed() {
                            let wave = wave.to_directed().unwrap();
                            if let Method::Cmd(CmdMethod::Log) = wave.core().method {
                                if let Substance::Log(log) = wave.core().body.clone() {
                                    if wave.to().is_single() {
                                        let to = wave.to().to_single().unwrap();
                                        if to.point == Point::global_logger() {
                                         membrane.logger.handle(log)
                                        }
                                    }
                                }
                            }

                        match wave {
                            DirectedWave::Ping(ping) => {
                                let proto: DirectedProto = ping.into();
                                let pong = transmitter.ping(proto);
                                let host = host.read().unwrap();
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
            }),

        } };

        let instance = match ext_imports {
            None => Instance::new(&module, &imports)?,
            Some(ext_imports) => {
                let imports = imports.chain_back(ext_imports);
                Instance::new(&module, &imports)?
            }
        };

        let membrane = Arc::new(WasmMembrane {
            instance: instance,
            init,
            name,
            platform,
            logger,
        });

        {
            host.write().unwrap().membrane = Option::Some(Arc::downgrade(&membrane));
        }

        return Ok(membrane);
    }
}

pub struct BufferLock<P>
where
    P: HostPlatform,
{
    id: i32,
    membrane: Arc<WasmMembrane<P>>,
}

impl<P> BufferLock<P>
where
    P: HostPlatform,
{
    pub fn new(id: i32, membrane: Arc<WasmMembrane<P>>) -> Self {
        BufferLock {
            id: id,
            membrane: membrane,
        }
    }

    pub fn id(&self) -> i32 {
        self.id.clone()
    }

    pub fn release(&self) -> Result<(), P::Err> {
        self.membrane.mechtron_guest_dealloc_buffer(self.id)?;
        Ok(())
    }
}

impl<P> Drop for BufferLock<P>
where
    P: HostPlatform,
{
    fn drop(&mut self) {
        self.release().unwrap_or(());
    }
}
