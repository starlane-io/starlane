use crate::error::Error;
use crate::mechtron::portal_client::MechtronSkel;
use crate::util::AsyncHashMap;
use mesh_portal_api_client::{Inlet, PortalSkel, PrePortalSkel, ResourceSkel};
use mesh_portal_serde::version::latest::id::Address;
use mesh_portal_serde::version::latest::messaging::{Request, Response};
use mesh_portal_serde::version::latest::portal;
use std::convert::TryFrom;
use std::future::Future;
use std::ops::Deref;
use std::sync::Arc;
use std::thread::Thread;
use tokio::sync::{mpsc, oneshot};
use wasm_membrane_host::membrane::WasmMembrane;
use wasmer::{Function, Module};

#[derive(Debug, Clone)]
pub enum MechtronCall {
    In(mechtron_common::inlet::Frame),
    Out(mechtron_common::outlet::Frame),
    Request {
        request: Request,
        tx: oneshot::Sender<Result<Option<Response>, Error>>,
    },
}

pub struct MechtronRequest {
    pub request: Request,
    pub tx: oneshot::Sender<Response>
}


#[derive(Clone)]
pub struct WasmSkel {
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
    InRequest(i32),
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
    pub fn new(module: Arc<Module>, skel: PrePortalSkel ) -> Result<Self, Error> {
        let (tx, mut rx) = mpsc::channel(1024);
        let skel = WasmSkel {
            pre_portal_skel,
            tx: tx.clone(),
        };
        let env = Env { tx };

        let mechtron_send_request =
            Function::new_native_with_env(module.store(), env, |env: &Env, request: i32| {
                let (tx, rx) = oneshot::channel();
                env.tx.try_send(MembraneExtCall::InRequest(request));
            });

        let imports = imports! {
                "env" => {

            "mechtron_inlet_frame"=>Function::new_native_with_env(module.store(),env,|env:&Env,frame:i32| {
                    let env = env.clone();
                    tokio::spawn( async move {
                       env.tx.send( MembraneExtCall::InFrame(frame) ).await;
                    });
                }),
              "mechtron_send_request"=>mechtron_send_request
            },

        };
        let membrane = WasmMembrane::new_with_init_and_imports(
            module,
            "mechtron_init".to_string(),
            Option::Some(imports),
        )?;
        let ext = Self { membrane, skel };

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
                            match process(&ext,  frame).await {
                                Ok(_) => {}
                                Err(err) => {
                                    eprintln!("error: {}", err.to_string());
                                }
                            }
                        }
                        MembraneExtCall::InRequest(request) => {
                            let ext = ext.clone();
                            tokio::spawn( async move {
                                fn process(
                                    ext: &WasmMembraneExt,
                                    request: i32,
                                ) -> Result<(), Error> {
                                    let request = ext.membrane.consume_buffer(request)?;
                                    let request: Request = bincode::deserialize(request.as_slice())?;
                                    let response = ext.skel.api().exchange(request).await;
                                    let func = ext
                                        .membrane
                                        .instance
                                        .exports
                                        .get_native_function::<i32, ()>("mechtron_handle_response")?;
                                    let response = bincode::serialize(&response)?;
                                    let response = ext.membrane.write_buffer(&response)?;
                                    func.call(response)?;
                                    Ok(())
                                }

                                match process(&ext,  request) {
                                    Ok(_) => {}
                                    Err(error) => {
                                        println!("error: {}", error.to_string());
                                    }
                                }
                            });
                        }
                    }
                }
            });
        }

        Ok(ext)
    }

    pub fn handle_frame(&self, frame: mechtron_common::outlet::Frame){
        fn process(ext: &WasmMembraneExt, frame: mechtron_common::outlet::Frame) -> Result<(), Error> {
            let func = ext
                .membrane
                .instance
                .exports
                .get_native_function::<i32, ()>("mechtron_handle_frame")?;
            let frame = bincode::serialize(&frame)?;
            let frame = ext.membrane.write_buffer(&frame)?;
            func.call(frame)?;
            Ok(())
        }
        match process(self, frame) {
            Ok(_) => {},
            Err(err) => {
                eprintln!("{}",err.to_string());
            }
        }
    }

    pub async fn handle_request(&self, request: Request) -> Response {
        fn process(ext: &WasmMembraneExt, request: Request) -> Result<Response, Error> {
            let func = ext
                .membrane
                .instance
                .exports
                .get_native_function::<i32, i32>("mechtron_handle_request")?;
            let request = bincode::serialize(&request)?;
            let request = ext.membrane.write_buffer(&request)?;
            let response: i32 = func.call(request)?;
            let response = ext.membrane.consume_buffer(response)?;
            let response: Response = bincode::deserialize(&response)?;
            Ok(response)
        }
        match process(self, request.clone()) {
            Ok(response) => response,
            Err(err) => {
                let response = request.fail(err.to_string());
                response
            }
        }
    }

}
