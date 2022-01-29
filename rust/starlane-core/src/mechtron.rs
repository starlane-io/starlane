use crate::cache::{ArtifactCaches, ArtifactItem};
use crate::config::bind::BindConfig;
use crate::config::mechtron::MechtronConfig;
use crate::config::wasm::Wasm;
use crate::error::Error;
use crate::mesh;
use crate::starlane::api::StarlaneApi;
use mesh_portal_api::message::Message;
use mesh_portal_serde::version::latest;
use mesh_portal_serde::version::v0_0_1::util::ConvertFrom;
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc;
use wasm_membrane_host::membrane::{WasmMembrane, WasmHost};

use crate::mesh::serde::messaging::{Exchange, ExchangeId};
use futures::SinkExt;
use mesh_portal_serde::version::latest::portal::inlet;
use mesh_portal_serde::version::latest::portal::outlet;
use mesh_portal_serde::version::latest::util::unique_id;
use mesh_portal_serde::version::v0_0_1::id::Address;
use mesh_portal_serde::version::v0_0_1::messaging::ExchangeType;
use std::collections::HashMap;
use std::convert::TryInto;
use std::sync::mpsc::Sender;
use std::time::Duration;
use tokio::sync::oneshot;
use tokio::sync::oneshot::error::RecvError;
use wasmer::{Function, Module, {imports, ImportObject}, Store};
use mesh_portal_serde::version::v0_0_1::generic::id::KindParts;
use mesh_portal_serde::version::v0_0_1::generic::entity::request::ReqEntity;
use mesh_portal_serde::version::v0_0_1::generic::portal::inlet::{Request, Frame};
use std::future::Future;
use crate::util::AsyncHashMap;
use std::ops::Deref;
use mesh_portal_serde::version::v0_0_1::generic::payload::Payload;
use mesh_portal_api_client::{ResourceCtrl, PortalSkel, ResourceCtrlFactory, ResourceSkel};
use mesh_portal_serde::version::v0_0_1::config::{Config, ResourceConfigBody};

#[derive(Clone)]
pub struct MechtronShell {
    pub skel: MechtronSkel,
}

impl MechtronShell {

    pub fn new(skel: MechtronSkel, rx: mpsc::Receiver<Call>) -> Self {

        let runner = MechtronRunner::new(skel.clone(),rx);
        tokio::spawn(async move {
            runner.run().await;
        });

        Self { skel }
    }
}

#[async_trait]
impl ResourceCtrl for MechtronShell {
    async fn outlet_frame(
        &self,
        frame: outlet::Frame
    ) -> Result<Option<inlet::Response>, anyhow::Error> {
        let (tx, rx) = oneshot::channel();
        self.skel.tx.send(Call::OutletFrame(frame)).await?;
        Ok(rx.await?)
    }
}


#[derive(Debug,Clone)]
pub enum Call {
    InletFrame(inlet::Frame),
    OutletFrame(outlet::Frame)
}

struct ExchangeInfo {
    pub tx: oneshot::Sender<inlet::Response>,
    pub core_exchange_id: ExchangeId,
    pub requester: latest::id::Address,
    pub responder: latest::id::Address,
}

#[derive(Clone)]
pub struct MechtronSkel {
    pub config: ArtifactItem<MechtronConfig>,
    pub wasm: ArtifactItem<Wasm>,
    pub bind: ArtifactItem<BindConfig>,
    pub tx: mpsc::Sender<Call>,
    pub membrane: MembraneExt,
    pub resource_skel: ResourceSkel,
}

#[derive(Clone)]
pub struct MechtronTemplate {
    pub config: ArtifactItem<MechtronConfig>,
    pub wasm: ArtifactItem<Wasm>,
    pub bind: ArtifactItem<BindConfig>,
}

impl MechtronTemplate {
    pub fn new(
        config: ArtifactItem<MechtronConfig>,
        caches: &ArtifactCaches,
    ) -> Result<Self, Error> {
        let skel = Self {
            config: config.clone(),
            wasm: caches.wasms.get(&config.wasm.address).ok_or(format!(
                "could not get referenced Wasm: {}",
                config.wasm.address.to_string()
            ))?,
            bind: caches
                .bind_configs
                .get(&config.bind.address)
                .ok_or::<Error>(
                    format!(
                        "could not get referenced BindConfig: {}",
                        config.wasm.address.to_string()
                    )
                    .into(),
                )?,
        };

        Ok(skel)
    }
}



impl MechtronSkel {


}

pub struct Factory {
    template: MechtronTemplate,
    membrane: MembraneExt
}

impl ResourceCtrlFactory for Factory {
    fn matches(&self, config: Config<ResourceConfigBody>) -> bool {
        todo!()
    }

    fn create(&self, resource_skel: ResourceSkel) -> Result<Arc<dyn ResourceCtrl>, anyhow::Error> {
        let (tx, rx) = mpsc::channel(1024);
        let skel = MechtronSkel {
            config: self.template.config.clone(),
            wasm: self.template.wasm.clone(),
            bind: self.template.bind.clone(),
            tx,
            membrane: self.membrane.clone(),
            resource_skel
        };
        Ok(Arc::new(MechtronShell::new(skel, rx)))
    }
}


pub struct MechtronRunner {
    skel: MechtronSkel,
    rx: mpsc::Receiver<Call>,
}

impl MechtronRunner {
    pub fn new(skel: MechtronSkel, rx: mpsc::Receiver<Call>)-> Self {
        Self {
            skel,
            rx
        }
    }

    async fn process( &self, call: Call ) -> Result<(),Error> {
        match call {
            Call::InletFrame(frame) => {
                self.skel.resource_skel.portal.inlet.inlet_frame(frame);
            }
            Call::OutletFrame(frame) => {
                let func = self.skel
                    .membrane
                    .instance
                    .exports
                    .get_native_function::<i32, i32>("mechtron_outlet_frame")?;
                if let outlet::Frame::Request(request) = &frame {
                    let frame = bincode::serialize(&frame )?;
                    let frame= self.skel.membrane.write_buffer(&frame)?;
                    let response: i32 = func.call(frame)?;
                    if response > 0 {
                        let response = self.skel.membrane.consume_buffer(response).unwrap();
                        let response: inlet::Response = bincode::deserialize(&response)?;
                        self.skel.tx.send( Call::InletFrame(inlet::Frame::Response(response))).await;
                    }
                } else {
                    let frame = bincode::serialize(&frame )?;
                    let frame= self.skel.membrane.write_buffer(&frame)?;
                    func.call(frame)?;
                }
            }
        }
        Ok(())
    }

    pub async fn run(mut self) {
        {
//            let mut exchanger = HashMap::new();
            while let Option::Some(call) = self.rx.recv().await {
                match self.process(call).await {
                    Ok(_) => {},
                    Err(err) => {
                        eprintln!("{}",err.to_string())
                    }
                }
            }
        }


    }
}


#[derive(Clone,WasmerEnv)]
pub struct Env {
    pub tx: mpsc::Sender<MembraneExtCall>,
}

pub enum MembraneExtCall {
   InletFrame(i32)
}


#[derive(Clone)]
pub struct MembraneExt {
    pub membrane: Arc<WasmMembrane>,
    pub map: AsyncHashMap<Address,mpsc::Sender<Call>>
}

impl Deref for MembraneExt {
    type Target = Arc<WasmMembrane>;

    fn deref(&self) -> &Self::Target {
        &self.membrane
    }
}

impl MembraneExt {
    pub fn new(module: Arc<Module>) -> Result<Self,Error>{
        let map = AsyncHashMap::new();
        let (tx,mut rx) = mpsc::channel(1024);
        let env = Env {
            tx
        };


        let imports = imports! {
                "env" => {

            "mechtron_inlet_frame"=>Function::new_native_with_env(module.store(),env,|env:&Env,request:i32| {
                    let env = env.clone();
                    tokio::spawn(async move {
                       env.tx.send(MembraneExtCall::InletFrame(request)).await;
                    });
                })
            }
        };
        let membrane = WasmMembrane::new_with_init_and_imports(module, "mechtron_init".to_string(), Option::Some(imports) )?;
        let ext = Self{
            membrane,
            map
        };

        {
            let ext = ext .clone();
            tokio::spawn(async move {
                while let Option::Some(call) = rx.recv().await {
                    match call {
                        MembraneExtCall::InletFrame(buffer) => {
                            async fn process( ext: &MembraneExt, buffer: i32 ) -> Result<(),Error> {
                                let buffer = ext.membrane.consume_buffer(buffer)?;
                                let frame: inlet::Frame = bincode::deserialize(buffer.as_slice())?;
                                if let Option::Some(from) = frame.from() {
                                    let tx = ext.map.get( from ).await?.ok_or::<Error>("cannot find mechtron tx".into())?;
                                    tx.send(Call::InletFrame(frame)).await;
                                } else {
                                    match frame {
                                        Frame::Log(log) => {println!("{}",log.to_string())}
                                        _ => {}
                                    }
                                }

                                Ok(())
                            }
                            match process(&ext,buffer).await {
                                Ok(_) => {}
                                Err(err) => {
                                    eprintln!("error: {}", err.to_string() );
                                }
                            }
                        }
                    }
                }
            });
        }

        Ok(ext)
    }

    pub fn add( &self, address: Address, tx: mpsc::Sender<Call>) {
        let map = self.map.clone();
        tokio::spawn(async move {
            map.put(address, tx).await;
        });
    }

    pub fn remove( &self, address: Address) {
        let map = self.map.clone();
        tokio::spawn(async move {
            map.remove(address ).await;
        });
    }
}