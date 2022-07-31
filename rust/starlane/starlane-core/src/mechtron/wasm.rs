/*use crate::error::Error;
use crate::mechtron::portal_client::MechtronSkel;
use crate::mesh_portal_uuid;
use crate::util::AsyncHashMap;
use mesh_portal_api_client::{Inlet, PortalSkel, PrePortalSkel, ParticleSkel};
use mesh_portal::version::latest::id::Point;
use mesh_portal::version::latest::messaging::{ReqShell, RespShell};
use mesh_portal::version::latest::portal;
use std::convert::TryFrom;
use std::future::Future;
use std::ops::Deref;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::Thread;
use threadpool::ThreadPool;
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::{mpsc, oneshot};
use wasm_membrane_host::membrane::WasmMembrane;
use wasmer::{Function, Module};

#[derive(Clone)]
pub struct WasmSkel {
    pub name: String,
    pub pre_portal_skel: PrePortalSkel,
    pub tx: mpsc::Sender<MembraneExtCall>,
}

impl Deref for WasmSkel {
    type Target = PrePortalSkel;

    fn deref(&self) -> &Self::Target {
        &self.pre_portal_skel
    }
}

#[derive(Clone, WasmerEnv)]
pub struct Env {
    pub tx: mpsc::Sender<MembraneExtCall>,
}

pub enum MembraneExtCall {
    InFrame(i32),
    InRequest {
        request: i32,
        mutex: Arc<Mutex<i32>>,
        condvar: Arc<Condvar>,
    },
    WriteString{
        string: String,
        mutex: Arc<Mutex<i32>>,
        condvar: Arc<Condvar>,
    }
}

#[derive(Clone)]
pub struct WasmMembraneExt {
    pub membrane: Arc<WasmMembrane>,
    pub skel: WasmSkel,
    pub pool: Arc<Mutex<ThreadPool>>,
}

impl Deref for WasmMembraneExt {
    type Target = Arc<WasmMembrane>;

    fn deref(&self) -> &Self::Target {
        &self.membrane
    }
}

impl WasmMembraneExt {
    pub fn new(module: Arc<Module>, name: String, pre_portal_skel: PrePortalSkel) -> Result<Self, Error> {
        let (tx, mut rx) = mpsc::channel(1024);
        let skel = WasmSkel {
            name: name.clone(),
            pre_portal_skel,
            tx: tx.clone(),
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

        let mechtron_unique_id = Function::new_native_with_env(module.store(),env.clone(), |env:&Env| -> i32 {
            let unique_id = uuid::Uuid::new_v4().to_string();
            let mutex = Arc::new(Mutex::new(0));
            let condvar = Arc::new(Condvar::new());
            match env.tx.try_send(MembraneExtCall::WriteString{
                string: unique_id,
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
        });

        let imports = imports! {
          "env" => {
            "mechtron_inlet_frame"=>Function::new_native_with_env(module.store(),env.clone(),|env:&Env,frame:i32| {
                    let env = env.clone();
                    tokio::spawn( async move {
                       env.tx.send( MembraneExtCall::InFrame(frame) ).await;
                    });
                }),
             "mechtron_inlet_request"=>mechtron_inlet_request,
             "mechtron_unique_id"=>mechtron_unique_id
           },
        };
        let membrane = WasmMembrane::new_with_init_and_imports(
            module,
            "mechtron_init".to_string(),
            name,
            Option::Some(imports),
        )?;
        let pool = Arc::new(Mutex::new(ThreadPool::new(10)));
        let ext = Self {
            membrane,
            skel,
            pool,
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

    pub fn handle_outlet_frame(&self, frame: mechtron_common::outlet::Frame) {
        fn process(
            ext: &WasmMembraneExt,
            frame: mechtron_common::outlet::Frame,
        ) -> Result<(), Error> {
if let mechtron_common::outlet::Frame::Assign(_) = frame {
    println!("OUTLET FRAME: ASSIGN!");
}

            let func = ext
                .membrane
                .instance
                .exports
                .get_native_function::<i32, ()>("mechtron_outlet_frame")?;
            let frame = bincode::serialize(&frame)?;
            let frame = ext.membrane.write_buffer(&frame)?;
            func.call(frame)?;
            Ok(())
        }

        let ext = self.clone();
        let pool = self.pool.lock().expect("expected ThreadPool");
        pool.execute(move || match process(&ext, frame) {
            Ok(_) => {}
            Err(err) => {
                eprintln!("{}", err.to_string());
            }
        });
    }

    pub async fn handle_outlet_request(&self, request: ReqShell) -> RespShell {
        fn process(ext: &WasmMembraneExt, request: ReqShell) -> Result<RespShell, Error> {
            let func = ext
                .membrane
                .instance
                .exports
                .get_native_function::<i32, i32>("mechtron_outlet_request")?;
            let request = bincode::serialize(&request)?;
            let request = ext.membrane.write_buffer(&request)?;
            let response: i32 = func.call(request)?;
            let response = ext.membrane.consume_buffer(response)?;
            let response: RespShell = bincode::deserialize(&response)?;
            Ok(response)
        }

        let (tx, rx) = oneshot::channel();

        {
            let ext = self.clone();
            let pool = self.pool.lock().expect("expected ThreadPool");
            let request = request.clone();
            pool.execute(move || {
                let response = match process(&ext, request.clone()) {
                    Ok(response) => response,
                    Err(err) => {
                        let response = request.fail(err.to_string().as_str());
                        response
                    }
                };
                tx.send(response);
            });
        }

        match rx.await {
            Ok(response) => response,
            Err(err) => request.fail(err.to_string().as_str()),
        }
    }
}


 */
