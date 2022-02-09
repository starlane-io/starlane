use std::collections::HashMap;
use std::process::Child;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use dashmap::DashMap;
use mesh_portal_api_server::{MeshRouter, MuxCall, PortalAssignRequestHandler, PortalCall, PortalMuxerEvent};

use crate::artifact::ArtifactRef;
use crate::error::Error;
use crate::resource::{ArtifactKind, ResourceType, ResourceAssign, AssignResourceStateSrc, Kind};
use crate::star::core::resource::manager::ResourceManager;
use crate::star::core::resource::state::StateStore;
use crate::star::{PortalEvent, StarSkel};
use crate::util::AsyncHashMap;
use crate::message::delivery::Delivery;
use mesh_portal_serde::version::latest::command::common::StateSrc;
use mesh_portal_serde::version::latest::config;
use mesh_portal_serde::version::latest::config::{Assign, Config};
use mesh_portal_serde::version::latest::id::Address;
use mesh_portal_serde::version::latest::messaging::{Request, RequestExchange, Response};
use mesh_portal_serde::version::latest::payload::{Payload, PayloadPattern, Primitive};
use mesh_portal_serde::version::latest::portal;
use mesh_portal_serde::version::latest::portal::Exchanger;
use mesh_portal_serde::version::latest::portal::inlet::AssignRequest;
use mesh_portal_serde::version::latest::resource::Properties;
use mesh_portal_tcp_server::{PortalServer, PortalTcpServer, TcpServerCall};
use mesh_portal_versions::version::v0_0_1::config::ResourceConfigBody;
use mesh_portal_versions::version::v0_0_1::pattern::consume_data_struct_def;
use mesh_portal_versions::version::v0_0_1::util::ValueMatcher;
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::sync::broadcast::Receiver;
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot::error::RecvError;
use crate::command::cli::outlet;
use crate::command::cli::outlet::Frame;
use crate::command::execute::CommandExecutor;
use crate::config::config::MechtronConfig;
use crate::config::parse::replace::substitute;

use crate::fail::Fail;
use crate::mechtron::process::launch_mechtron_process;
use crate::mechtron::wasm::MechtronRequest;
use crate::message::Reply;
use crate::starlane::api::StarlaneApi;

lazy_static! {
    pub static ref STARLANE_MECHTRON_PORT: usize = std::env::var("STARLANE_MECHTRON_PORT").unwrap_or("4345".to_string()).parse::<usize>().unwrap_or(4345);
}

pub struct MechtronManager {
    skel: StarSkel,
    processes: AsyncHashMap<String, Child>,
    portals: AsyncHashMap<String, mpsc::Sender<PortalCall>>,
    mechtrons: AsyncHashMap<String, mpsc::Sender<PortalCall>>,
    resource_type: ResourceType,
    server: Sender<TcpServerCall>,
    portal_broadcast_tx: broadcast::Sender<String>
}

impl MechtronManager {
    pub async fn new(skel: StarSkel, resource_type:ResourceType) -> Self {
        let server = PortalTcpServer::new(STARLANE_MECHTRON_PORT.clone(), Box::new(MechtronPortalServer::new(skel.clone() )));
        let (portal_broadcast_tx,_) = broadcast::channel(1024);
        let mut rtn = MechtronManager {
            skel: skel.clone(),
            processes: AsyncHashMap::new(),
            portals: AsyncHashMap::new(),
            mechtrons: AsyncHashMap::new(),
            resource_type,
            server,
            portal_broadcast_tx,
        };
        rtn.init();
        rtn
    }
}

impl MechtronManager {
    fn init(&mut self) {
        let portals = self.portals.clone();
        let mechtrons = self.mechtrons.clone();
        let server = self.server.clone();
        let portal_broadcast_tx = self.portal_broadcast_tx.clone();
        tokio::spawn(async move {
            let (tx,rx) = oneshot::channel();
            server.send( TcpServerCall::GetPortalMuxerBroadcaster(tx)).await;
            match rx.await {
                Ok(mut rx) => {
                    while let Ok(event) = rx.recv().await {
                        match event {
                            PortalMuxerEvent::PortalAdded { info, tx } => {
                                portals.put( info.portal_key.clone(), outlet_tx.clone() );
                                portal_broadcast_tx.send(info.portal_key);
                            }
                            PortalMuxerEvent::PortalRemoved(info) => {
                                portals.remove( info.portal_key );
                            }
                            PortalMuxerEvent::ResourceAssigned { stub, tx } => {
                                mechtrons.put( stub.address.to_string(), tx );
                            }
                            PortalMuxerEvent::ResourceRemoved(address) => {
                                mechtrons.remove( address.to_string() );
                            }
                        }
                    }
                }
                Err(err) => {
                    error!("{}",err.to_string());
                }
            }
        });
    }

    fn complete_assign() {

    }
}

#[async_trait]
impl ResourceManager for MechtronManager {



    async fn assign(
        &self,
        assign: ResourceAssign,
    ) -> Result<(), Error> {
        match assign.state {
            StateSrc::Stateless => {}
            _ => {
                return Err("currently only supporting stateless mechtrons".into());
            }
        };

        let config_address = assign.stub.properties.get(&"config".to_string() ).ok_or(format!("'config' property required to be set for {}", self.resource_type.to_string() ))?.value.as_str();
        let config_address = Address::from_str(config_address)?;

        let config_artifact_ref = ArtifactRef {
          address:config_address.clone(),
          kind: ArtifactKind::ResourceConfig
        };

        let caches = self.skel.machine.cache( &config_artifact_ref ).await?;
        let config = caches.resource_configs.get(&config_address).ok_or::<Error>(format!("expected mechtron_config").into())?;
        let config = MechtronConfig::new(config, assign.stub.address.clone() );

        let api = StarlaneApi::new( self.skel.surface_api.clone(), assign.stub.address.clone() );
        let substitution_map = config.substitution_map()?;
        for mut command_line in config.install {
            command_line = substitute(command_line.as_str(), &substitution_map)?;
            println!("INSTALL: '{}'",command_line);
            let mut output_rx = CommandExecutor::exec_simple(command_line,assign.stub.clone(), api.clone() );
            while let Some(frame) = output_rx.recv().await {
                match frame {
                    outlet::Frame::StdOut(out) => {
                        println!("{}",out);
                    }
                    outlet::Frame::StdErr(out) => {
                        eprintln!("{}", out);
                    }
                    outlet::Frame::EndOfCommand(code) => {
                        if code != 0 {
                            eprintln!("install error code: {}",code);
                        }
                    }
                }
            }
        }

        let wasm_src = config.wasm_src()?;
        if let Option::Some(tx) = self.portals.get(wasm_src.to_string() ).await? {
            let assign = Assign {
                config: Config::new( ResourceConfigBody::Named(config.mechtron_name()?)),
                stub: assign.stub.clone()
            };
           tx.send(PortalCall::Assign(assign));
        } else {
            let mut portal_broadcast_rx = self.portal_broadcast_tx.subscribe();
            let wasm_src = wasm_src.clone();
            let handle = tokio::spawn(async move {
                while let Ok(portal_key) = portal_broadcast_rx.recv().await {
                    if portal_key == wasm_src.to_string() {
                        break;
                    }
                }
            });

            if !self.processes.contains(wasm_src.to_string() ) {
                let child = launch_mechtron_process(wasm_src)?;
                self.processes.put( wasm_src.to_string(), child ).await;
            }

            tokio::time::timeout( Duration::from_secs(60),handle ).await??;

            if let Option::Some(tx) = self.portals.get(wasm_src.to_string() ).await? {
                let assign = Assign {
                    config: Config::new( ResourceConfigBody::Named(config.mechtron_name()?)),
                    stub: assign.stub.clone()
                };
                tx.send(PortalCall::Assign(assign));
            } else {
                return Err(format!("expected portal to be available: '{}'", wasm_src.to_string() ).into());
            }
        }

        Ok(())
    }

    async fn handle_request(&self, request: Request ) -> Response {

            async fn get_mechtron_portal( mechtrons: &AsyncHashMap<String,mpsc::Sender<PortalCall>>) -> Result<mpsc::Sender<PortalCall>,Error> {
                Ok(mechtrons.get(request.to.to_string() ).await?.ok_or("not present")?)
            }

            let mechtron_portal =  match get_mechtron_portal(&self.mechtrons){
                Ok(mechtron_portal) => {mechtron_portal}
                Err(err) => {
                    return request.fail(err.to_string() );
                }
            };

            let (exchange,mut rx) = RequestExchange::new(request.clone());
            mechtron_portal.send( PortalCall::Request(exchange)).await;

            match tokio::time::timeout( Duration::from_secs(30), rx ).await {
                Ok(Ok(response)) => {
                    response
                }
                _ => {
                    request.fail("timeout".to_string() )
                }
            }

    }


    fn resource_type(&self) -> ResourceType {
        self.resource_type.clone()
    }
}



impl MechtronStarVariant {
    fn init(&mut self) -> Result<(),Error> {

        self.server = Option::Some(server);
        Ok(())
    }
}

pub struct MechtronPortalServer {
    pub skel: StarSkel,
    pub request_assign_handler: Arc<dyn PortalAssignRequestHandler>
}

impl MechtronPortalServer {
    pub fn new(skel: StarSkel) -> Self {
        Self {
            skel: skel.clone(),
            request_assign_handler: Arc::new(MechtronPortalAssignRequestHandler::new(skel) )
        }
    }
}

#[async_trait]
impl PortalServer for MechtronPortalServer {
    fn flavor(&self) -> String {
        "mechtron".to_string()
    }

    fn logger(&self) -> fn(&str) {
        test_logger
    }

    fn router_factory(&self) -> Box<dyn Router> {
        Box::new(StarlaneMeshRouter { skel: self.skel.clone()  })
    }

    fn portal_request_handler(&self) -> Arc<dyn PortalAssignRequestHandler> {
        self.request_assign_handler.clone()
    }
}


fn test_logger(message: &str) {
    println!("{}", message);
}

pub struct StarlaneMeshRouter {
    skel: StarSkel,
}

impl MeshRouter for StarlaneMeshRouter {
    fn request(&self, exchange: RequestExchange) {
        let skel = self.skel.clone();
        tokio::spawn( async move {
            let response = skel.messaging_api.exchange(exchange.request).await;
            exchange.tx.send(response);
        });
    }
}

#[derive(Debug)]
pub struct MechtronPortalAssignRequestHandler {
    skel: StarSkel
}

impl MechtronPortalAssignRequestHandler {
    pub fn new(skel: StarSkel)-> Self {
        MechtronPortalAssignRequestHandler {
            skel
        }
    }
}

#[async_trait]
impl PortalAssignRequestHandler for MechtronPortalAssignRequestHandler {

}
