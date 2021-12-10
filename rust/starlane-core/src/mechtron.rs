use crate::cache::{ArtifactItem, ArtifactCaches};
use crate::config::mechtron::MechtronConfig;
use crate::config::wasm::Wasm;
use crate::config::bind::{BindConfig};
use crate::error::Error;
use wasm_membrane_host::membrane::WasmMembrane;
use std::sync::Arc;
use tokio::sync::mpsc;
use mesh_portal_api::message::Message;
use mechtron_common::version::latest::{guest, host};
use crate::mesh;
use mesh_portal_serde::version::latest;
use mesh_portal_serde::version::v0_0_1::util::ConvertFrom;
use crate::starlane::api::StarlaneApi;

use mesh_portal_serde::version::v0_0_1::id::Address;
use crate::mesh::serde::config::Info;
use mesh_portal_api_client::{PortalSkel, PortalCtrl};
use std::convert::TryInto;
use wasmer::{{ImportObject, imports}, Module};
use tokio::sync::oneshot;
use std::time::Duration;
use mesh_portal_serde::version::v0_0_1::messaging::ExchangeType;
use crate::mesh::serde::messaging::Exchange;
use std::collections::HashMap;
use futures::SinkExt;
use mesh_portal_serde::version::v0_0_1::generic::portal::inlet::Response;
use mesh_portal_serde::version::v0_0_1::generic::payload::Payload;
use tokio::sync::oneshot::error::RecvError;
use std::sync::mpsc::Sender;

#[derive(Clone)]
pub struct MechtronShell {
    pub skel: MechtronShellSkel,
    pub portal: PortalSkel
}

enum Call {
    GuestRequest { request: guest::Request, tx: oneshot::Sender<Result<Option<latest::portal::inlet::Response>,Error>> },
    HostResponse(host::Response),
    GuestResponse(GuestResponse),
    PortalOutletRequest(host::Request),
    PortalOutletResponse(latest::portal::outlet::Response)
}

struct ExchangeInfo {
    pub tx: oneshot::Sender<latest::portal::outlet::Response>,
    pub requester: latest::id::Identifier,
    pub responder: latest::id::Address,
}

#[derive(Clone)]
pub struct MechtronShellSkel {
    pub config: ArtifactItem<MechtronConfig>,
    pub wasm: ArtifactItem<Wasm>,
    pub bind: ArtifactItem<BindConfig>,
    pub tx: mpsc::Sender<Call>,
    pub membrane: Arc<WasmMembrane>
}

#[derive(Clone)]
pub struct MechtronShellTemplate {
    pub config: ArtifactItem<MechtronConfig>,
    pub wasm: ArtifactItem<Wasm>,
    pub bind: ArtifactItem<BindConfig>,
}

impl MechtronShellTemplate {
    pub fn new(config: ArtifactItem<MechtronConfig>, caches: &ArtifactCaches) -> Result<Self, Error> {
        let skel = Self {
            config,
            wasm: caches.wasms.get(&config.wasm.address).ok_or(format!("could not get referenced Wasm: {}", config.wasm.address.to_string()))?,
            bind: caches.bind_configs.get(&config.bind.address).ok_or::<Error>(format!("could not get referenced BindConfig: {}", config.wasm.address.to_string()).into())?,
        };

        Ok(skel)
    }
}

impl MechtronShellSkel {
    //            membrane: WasmMembrane::new_with_init(wasm.module.clone(), "mechtron_init".to_string())?
        pub fn new(template: MechtronShellTemplate, membrane: Arc<WasmMembrane> ) -> Result<Self, Error> {

        let (tx,mut rx) = mpsc::channel(128);

        let skel = MechtronShellSkel {
            config: template.config,
            wasm: template.wasm,
            bind: template.bind,
            membrane,
            tx
        };

        {
            let skel = skel.clone();
            tokio::spawn(async move {
                let mut exchanger = HashMap::new();
                while let Option::Some(call) = rx.recv() {
                    match call {
                        Call::GuestRequest { request, tx } => {
                         tokio::spawn( async move {
                             fn handle(request: guest::Request) -> Result<Option<latest::portal::inlet::Response>, Error> {
                                 let from = request.from.clone();
                                 let frame = guest::Frame::Request(request);
                                 let frame = bincode::serialize(&frame)?;
                                 let frame = skel.membrane.write_buffer(&frame)?;
                                 let response: i32 = func.call(frame)?;
                                 if response > 0 {
                                     let response = skel.membrane.consume_buffer(response).unwrap();
                                     let response: host::Response = bincode::deserialize(&response)?;

                                     let response = latest::portal::inlet::Response {
                                         id: latest::util::unique_id(),
                                         to: from,
                                         exchange: response.exchange,
                                         entity: ConvertFrom::convert_from(response.entity.clone())?
                                     };
                                     Ok(Option::Some(response))
                                 } else {
                                     Ok(Option::None)
                                 }
                             }
                             let result = handle(request);
                             tx.send(result);
                         });
                        }
                        Call::GuestResponse(response) => {
                            match exchanger.remove(&response.exchange ) {
                                Some(mut tx) => {
                                    tx.send(response);
                                }
                                None => {
                                    eprintln!("could not find exchanger for message");
                                }
                            }
                        }
                        Call::PortalOutletRequest(request) => {

                            let exchange = match request.exchange {
                                ExchangeType::Notification => {
                                    Exchange::Notification
                                }
                                ExchangeType::RequestResponse => {
                                    let exchange_id =latest::util::unique_id();
                                    let (tx,rx):(oneshot::Sender<latest::portal::outlet::Response>,oneshot::Receiver<latest::portal::outlet::Response>) = oneshot::channel();
                                    let exchange = ExchangeInfo {
                                        tx,
                                        requester: request.from.clone(),
                                        responder: request.to.clone()
                                    };

                                    exchanger.insert( exchange_id.clone(), exchange );

                                    tokio::spawn( async move {
                                       fn handle(response: Result<latest::portal::outlet::Response,RecvError>)->Result<guest::Response,Error> {
                                            let response = response?;
                                                let response = guest::Response {
                                                    to: response.to.try_into()?,
                                                    from: response.from.try_into()?,
                                                    entity: response.entity
                                                };
                                            Ok(response)
                                       }
                                       let response = handle( rx.await );
                                       match response {
                                            Ok(response) => {
                                                skel.tx.send( Call::GuestResponse(response) ).await;
                                            }
                                            Err(err) => {
                                                eprintln!("{}",err.to_string());
                                            }
                                       }
                                    });

                                    Exchange::RequestResponse(exchange_id)
                                }
                            };

                            let request = latest::portal::inlet::Request {
                                id: request.id,
                                to: vec![request.to.into()],
                                entity: request.entity,
                                exchange: exchange
                            };
                        }
                    }
                }
            });
        }

        Ok(skel)
    }

    fn create_import_object(&self, module: Arc<Module>) -> ImportObject {
       imports! {
            "env"=>{

        "mechtron_host_request"=>Function::new_native_with_env(module.store(),skel.clone(),|skel:&MechtronShellSkel,request:i32| {
                fn handle(skel: &MechtronShellSkel, request:i32) -> Result<(),Error> {
                  let request = self.membrane.consume_buffer(buffer);
                  let request: host::Request = bincode::deserialize(request.as_slice());
                  skel.tx.try_send(Call::PortalOutletRequest(request))?;
                }
                match handle( skel, request)
                {
                    Ok(_) => {},
                    Err(err) => {
                      eprintln!("mechtron_host_request ERROR: {}",err.to_string());
                    }
                }
            })
        }
    }

    }
}

impl MechtronShell {
    pub fn new_shared_wasm_factory(skel: MechtronShellSkel) -> Result<impl Fn<PortalSkel,Output=Box<dyn PortalCtrl>>, Error> {

        struct Factory {
            skel: MechtronShellSkel
        }

        impl FnOnce<PortalSkel> for Factory {
            type Output = Box<dyn PortalCtrl>;

            extern "rust-call" fn call_once(self, portal: PortalSkel) -> Self::Output {
                Box::new(MechtronShell::new( self.skel, portal ))
            }
        }

        impl FnMut<PortalSkel> for Factory {
            extern "rust-call" fn call_mut(&mut self, args: PortalSkel) -> Self::Output {
                Box::new(MechtronShell::new( self.skel.clone(), portal ))
            }
        }

        impl Fn<PortalSkel> for Factory {
            extern "rust-call" fn call(&self, args: PortalSkel) -> Self::Output {
                Box::new(MechtronShell::new( self.skel.clone(), portal ))
            }
        }

        let factory = Factory {
            skel
        };

        Ok(factory)
    }

    pub fn new_isolated_wasm_factory(template: MechtronShellTemplate) -> Result<impl Fn<PortalSkel,Output=Result<Box<dyn PortalCtrl>,Error>>, Error> {

        struct Factory {
            template: MechtronShellTemplate
        }

        impl FnOnce<PortalSkel> for Factory {
            type Output = Result<Box<dyn PortalCtrl>,Error>;

            extern "rust-call" fn call_once(self, portal: PortalSkel) -> Self::Output {
              let membrane = WasmMembrane::new_with_init(self.template.wasm.module.clone(), "mechtron_init".to_string())?;
              let skel = MechtronShellSkel::new(self.template, membrane)?;
              Ok(Box::new(MechtronShell::new(skel,portal)))
            }
        }

        impl FnMut<PortalSkel> for Factory {
            extern "rust-call" fn call_mut(&mut self, args: PortalSkel) -> Self::Output {
                let membrane = WasmMembrane::new_with_init(self.template.wasm.module.clone(), "mechtron_init".to_string())?;
                let skel = MechtronShellSkel::new(self.template.clone(), membrane)?;
                Ok(Box::new(MechtronShell::new(skel,portal)))
            }
        }

        impl Fn<PortalSkel> for Factory {
            extern "rust-call" fn call(&self, args: PortalSkel) -> Self::Output {
                let membrane = WasmMembrane::new_with_init(self.template.wasm.module.clone(), "mechtron_init".to_string())?;
                let skel = MechtronShellSkel::new(self.template.clone(), membrane)?;
                Ok(Box::new(MechtronShell::new(skel,portal)))
            }
        }

        let factory = Factory {
           template
        };

        Ok(factory)
    }

    pub fn new(skel: MechtronShellSkel, portal: PortalSkel ) -> Self {

        Self {
            skel,
            portal,
        }
    }
}

#[async_trait]
impl PortalCtrl for MechtronShell{

    async fn handle(&self, message: Message) -> Result<Option<mesh::serde::portal::inlet::Response>,Error> {
        let func = self.membrane.instance.exports.get_native_function::<i32,i32>("mechtron_guest_frame")?;
                match message {
                    Message::Request(request) => {
                        let request = guest::Request {
                            to: self.info.address.clone(),
                            from: request.from,
                            entity: request.entity,
                            exchange: request.exchange
                        };

                        let (tx,rx) = oneshot::channel();
                        skel.tx.send(Call::GuestRequest {request, tx}).await?;
                        rx.await
                    }
                    Message::Response(response) => {

                    }
                }

        Ok(Option::None)

            }


    /*
    pub async fn http_request( &self, message: Message<HttpRequest>) -> Result<Option<HttpResponse>,Error> {
        let call = MechtronCall {
            mechtron: self.config.name.clone(),
            command: MechtronCommand::HttpRequest(message)
        };

        let string = serde_json::to_string(&call)?;
        info!("{}",string);
        let call = self.membrane.write_string(string.as_str())?;
        info!("message delivery to mechtron complete...{}", call);
        match self.membrane.instance.exports.get_native_function::<i32,i32>("mechtron_call"){

            Ok(func) => {
                match func.call(call)
                {
                    Ok(reply) => {

                        if reply > 0 {
                            let reply_json = self.membrane.consume_string(reply).unwrap();
                            let reply:MechtronResponse = serde_json::from_str(reply_json.as_str())?;
                            if let MechtronResponse::HttpResponse(reply)= reply {
                                info!("... HOST .... SENDING REPLY......");
                                Ok(Option::Some(reply))
                            }
                            else {
                                error!("MechtronResponse::PortReply not expected!");
                                Ok(Option::None)
                            }
                        }
                        else {
                            Ok(Option::None)
                        }

                    }
                    Err(error) => {
                        error!("wasm runtime error: {}",error );
                        Err("wasm runtime error".into())
                    }
                }
            }
            Err(error) => {
                error!("error when exporting function: mechtron_call" );
                Err("wasm export error".into())
            }
        }

    }*/



}
