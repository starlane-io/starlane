use std::ops::Deref;
use crate::driver::{
    Driver, DriverAvail, DriverCtx, DriverSkel, HyperDriverFactory, ItemHandler, ItemSphere,
};
use wasm_membrane_host::membrane::WasmMembrane;
use crate::star::HyperStarSkel;
use crate::Hyperverse;
use cosmic_universe::artifact::ArtRef;
use cosmic_universe::config::bind::BindConfig;
use cosmic_universe::kind::{BaseKind, Kind};
use cosmic_universe::loc::Point;
use cosmic_universe::parse::bind_config;
use cosmic_universe::selector::KindSelector;
use cosmic_universe::util::log;
use std::str::FromStr;
use std::sync::{Arc, Condvar};
use tokio::sync::{mpsc, Mutex};
use cosmic_universe::err::UniErr;
use cosmic_universe::frame::PrimitiveFrame;
use threadpool::ThreadPool;

lazy_static! {
    static ref HOST_BIND_CONFIG: ArtRef<BindConfig> = ArtRef::new(
        Arc::new(host_bind()),
        Point::from_str("GLOBAL::repo:1.0.0:/bind/host.bind").unwrap()
    );
}

fn host_bind() -> BindConfig {
    log(bind_config(
        r#"
    Bind(version=1.0.0)
    {
    }
    "#,
    ))
    .unwrap()
}

pub struct HostDriverFactory {
    pub avail: DriverAvail,
}

impl HostDriverFactory {
    pub fn new(avail: DriverAvail) -> Self {
        Self { avail }
    }
}

#[async_trait]
impl<P> HyperDriverFactory<P> for HostDriverFactory
where
    P: Hyperverse,
{
    fn kind(&self) -> KindSelector {
        KindSelector::from_base(BaseKind::Host)
    }

    async fn create(
        &self,
        skel: HyperStarSkel<P>,
        driver_skel: DriverSkel<P>,
        ctx: DriverCtx,
    ) -> Result<Box<dyn Driver<P>>, P::Err> {
        Ok(Box::new(HostDriver::new(self.avail.clone())))
    }
}

pub struct HostDriver {
    pub avail: DriverAvail,
}

#[handler]
impl HostDriver {
    pub fn new(avail: DriverAvail) -> Self {
        Self { avail }
    }
}

#[async_trait]
impl<P> Driver<P> for HostDriver
where
    P: Hyperverse,
{
    fn kind(&self) -> Kind {
        Kind::Host
    }

    async fn item(&self, point: &Point) -> Result<ItemSphere<P>, P::Err> {
        Ok(ItemSphere::Handler(Box::new(Host)))
    }
}

pub struct Host;

#[handler]
impl Host {}

#[async_trait]
impl<P> ItemHandler<P> for Host
where
    P: Hyperverse,
{
    async fn bind(&self) -> Result<ArtRef<BindConfig>, P::Err> {
        Ok(HOST_BIND_CONFIG.clone())
    }
}


#[derive(Clone)]
pub struct WasmSkel {
    pub pool: Arc<Mutex<ThreadPool>>,
}

#[derive(Clone, WasmerEnv)]
pub struct Env {
    pub tx: mpsc::Sender<MembraneExtCall>,
}

pub enum WasmCall {
    InFrame(PrimitiveFrame)
}


#[derive(Clone)]
pub struct WasmMembraneExt {
    pub membrane: Arc<WasmMembrane>,
    pub skel: WasmSkel,
}

impl Deref for WasmMembraneExt {
    type Target = Arc<WasmMembrane>;

    fn deref(&self) -> &Self::Target {
        &self.membrane
    }
}

impl WasmMembraneExt {
        pub fn new(module: Arc<Module>, pool: Arc<Mutex<ThreadPool>>) -> Result<Self, UniErr> {
        let (tx, mut rx) = mpsc::channel(1024);
        let skel = WasmSkel {
            pool: pool.clone()
        };
        let mut env = Env { tx };

        let mechtron_inlet_request = Function::new_native_with_env(
            module.store(),
            env.clone(),
            |env: &Env, request: i32| -> i32 {
                let mutex = Arc::new(Mutex::new(0));
                let condvar = Arc::new(Condvar::new());
                match env.tx.try_send(MembraneExtCall::InRequest {
                    request,
                    mutex: mutex.clone(),
                    condvar: condvar.clone(),
                }) {
                    Ok(_) => {
                        let mut lock = mutex.lock().unwrap();
                        while *lock == 0 {
                            lock = condvar.wait(lock).unwrap();
                        }
                        return lock.deref().clone();
                    }
                    Err(_) => {
                        return -1;
                    }
                }
            },
        );

        let imports = imports! {
          "env" => {
            "mechtron_inlet_frame"=>Function::new_native_with_env(module.store(),env.clone(),|env:&Env,frame:i32| {
                    let env = env.clone();
                    tokio::spawn( async move {
                       env.tx.send( MembraneExtCall::InFrame(frame) ).await;
                    });
                }),
           },
        };
        let membrane = WasmMembrane::new_with_init_and_imports(
            module,
            "mechtron_guest_init".to_string(),
            name,
            Option::Some(imports),
        )?;
        let ext = Self {
            membrane,
            skel,
        };

        {
            let ext = ext.clone();
            tokio::spawn(async move {
                while let Option::Some(call) = rx.recv().await {
                    match call {
                        MembraneExtCall::InFrame(frame) => {
                            async fn process(
                                ext: &WasmMembraneExt,
                                frame: i32,
                            ) -> Result<(), Error> {
                                let frame = ext.membrane.consume_buffer(frame)?;
                                let frame: mechtron_common::inlet::Frame =
                                    bincode::deserialize(frame.as_slice())?;
                                let frame: portal::inlet::Frame = frame.into();
                                ext.skel.inlet.inlet_frame(frame);
                                Ok(())
                            }
                            match process(&ext, frame).await {
                                Ok(_) => {}
                                Err(err) => {
                                    eprintln!("error: {}", err.to_string());
                                }
                            }
                        }
                        MembraneExtCall::InRequest {
                            request,
                            mutex,
                            condvar,
                        } => {
                            let ext = ext.clone();
                            tokio::spawn(async move {
                                async fn process(
                                    ext: &WasmMembraneExt,
                                    request: i32,
                                ) -> Result<i32, Error> {
                                    let request = ext.membrane.consume_buffer(request)?;
                                    let request: ReqShell =
                                        bincode::deserialize(request.as_slice())?;
                                    let response = ext.skel.api().exchange(request).await;
                                    let response = bincode::serialize(&response)?;
                                    let response = ext.membrane.write_buffer(&response)?;
                                    Ok(response)
                                }

                                let response = match process(&ext, request).await {
                                    Ok(response) => response,
                                    Err(error) => {
                                        println!("error: {}", error.to_string());
                                        -1
                                    }
                                };

                                let mut rtn = mutex.lock().unwrap();
                                *rtn = response;
                                // We notify the condvar that the value has changed.
                                condvar.notify_one();
                            });
                        }
                        MembraneExtCall::WriteString{
                            string,
                            mutex,
                            condvar,
                        } => {
                            let ext = ext.clone();
                            tokio::spawn(async move {
                                async fn process(
                                    ext: &WasmMembraneExt,
                                    string: String,
                                ) -> Result<i32, Error> {
                                    let string = ext.membrane.write_string(string.as_str())?;
                                   Ok(string)
                                }
                                let buffer_id = match process(&ext, string).await {
                                    Ok(buffer_id) => buffer_id,
                                    Err(error) => {
                                        println!("error: {}", error.to_string());
                                        -1
                                    }
                                };

                                let mut rtn = mutex.lock().unwrap();
                                *rtn = buffer_id;
                                // We notify the condvar that the value has changed.
                                condvar.notify_one();
                            });
                        }
                    }
                }
            });
        }

        ext.membrane.init();

        Ok(ext)
    }


}

