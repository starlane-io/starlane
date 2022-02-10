use std::collections::HashMap;
use std::process::Child;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use dashmap::DashMap;
use dashmap::mapref::one::Ref;
use mesh_portal_api_server::{Portal, PortalRequestHandler};

use crate::artifact::ArtifactRef;
use crate::error::Error;
use crate::resource::{ArtifactKind, ResourceType, ResourceAssign, AssignResourceStateSrc, Kind};
use crate::star::core::resource::manager::ResourceManager;
use crate::star::core::resource::state::StateStore;
use crate::star::{StarSkel};
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
use crate::message::Reply;
use crate::starlane::api::StarlaneApi;

lazy_static! {
    pub static ref STARLANE_MECHTRON_PORT: usize = std::env::var("STARLANE_MECHTRON_PORT").unwrap_or("4345".to_string()).parse::<usize>().unwrap_or(4345);
}

pub struct MechtronManager {
    skel: StarSkel,
    processes: DashMap<String, Child>,
    portals: Arc<DashMap<String,Portal>>,
    mechtrons: AsyncHashMap<Address, String>,
    resource_type: ResourceType,
    server: Sender<TcpServerCall>,
    portal_added_broadcast_tx: broadcast::Sender<String>,
}

impl MechtronManager {
    pub async fn new(skel: StarSkel, resource_type:ResourceType) -> Self {
        let portals = Arc::new(DashMap::new());
        let (portal_added_broadcast_tx,_) = broadcast::channel(1024);
        let server = PortalTcpServer::new(STARLANE_MECHTRON_PORT.clone(), Box::new(MechtronPortalServer::new(skel.clone(), portals.clone(), portal_added_broadcast_tx.clone() )));
        let mut rtn = MechtronManager {
            skel: skel.clone(),
            processes: DashMap::new(),
            portals,
            mechtrons: AsyncHashMap::new(),
            resource_type,
            server,
            portal_added_broadcast_tx
        };
        rtn.init();
        rtn
    }
}

impl MechtronManager {
    fn init(&mut self) {

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
        for command_line in &config.install {
            let command_line = substitute(command_line.as_str(), &substitution_map)?;
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
        if let Option::Some(portal) = self.portals.get(&wasm_src.to_string() ) {
            let portal_assign = Assign {
                config: Config {
                    body: ResourceConfigBody::Named(config.mechtron_name()?),
                    address: assign.stub.address.clone()
                },
                stub: assign.stub.clone()
            };
            portal.assign(portal_assign);
            self.mechtrons.put(assign.stub.address.clone(), portal.info.portal_key.clone() );
        } else {
            let mut portal_added_broadcast_tx = self.portal_added_broadcast_tx.subscribe();
            let handle = {
                let wasm_src = wasm_src.clone();
                tokio::spawn(async move {
                    while let Ok(portal_key) = portal_added_broadcast_tx.recv().await {
                        if portal_key == wasm_src.to_string() {
                            break;
                        }
                    }
                })
            };

            if !self.processes.contains_key(&wasm_src.to_string() ) {
                let child = launch_mechtron_process(wasm_src.clone())?;
                self.processes.insert( wasm_src.to_string(), child );
            }

            tokio::time::timeout( Duration::from_secs(60),handle ).await??;

            if let Option::Some(portal) = self.portals.get(&wasm_src.to_string() ) {
                let portal_assign = Assign {
                    config: Config {
                        body: ResourceConfigBody::Named(config.mechtron_name()?),
                        address: assign.stub.address.clone()
                    },
                    stub: assign.stub.clone()
                };
                portal.assign(portal_assign);
                self.mechtrons.put(assign.stub.address.clone(), portal.info.portal_key.clone() );
            } else {
                return Err(format!("expected portal to be available: '{}'", wasm_src.to_string() ).into());
            }
        }

        Ok(())
    }

    async fn handle_request(&self, request: Request ) -> Response {

            match self.mechtrons.get(request.to.clone() ).await {
                Ok(Some(portal_key)) => {
                    match self.portals.get(&portal_key ) {
                        Some(portal) => {
                            match tokio::time::timeout( Duration::from_secs(30), portal.handle_request(request.clone()) ).await {
                                Ok(response) => {
                                    response
                                }
                                _ => {
                                    request.fail("timeout".to_string() )
                                }
                            }
                        }
                        None => {
                            request.fail(format!("portal not found: '{}'",portal_key).into() )
                        }
                    }
                }
                Ok(None) => {
                    let to = request.to.to_string();
                    request.fail(format!("not found: '{}'",to ).into() )
                }
                Err(err) => {
                    request.fail(err.to_string().into() )
                }
            }
    }


    fn resource_type(&self) -> ResourceType {
        self.resource_type.clone()
    }
}

pub struct MechtronPortalServer {
    pub skel: StarSkel,
    pub request_assign_handler: Arc<dyn PortalRequestHandler>,
    portals: Arc<DashMap<String, Portal>>,
    portal_added_broadcast_tx: broadcast::Sender<String>,
}

impl MechtronPortalServer {
    pub fn new(skel: StarSkel, portals: Arc<DashMap<String, Portal>>, portal_added_broadcast_tx: broadcast::Sender<String>) -> Self {
        Self {
            skel: skel.clone(),
            request_assign_handler: Arc::new(MechtronPortalRequestHandler::new(skel) ),
            portals,
            portal_added_broadcast_tx
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

    fn portal_request_handler(&self) -> Arc<dyn PortalRequestHandler> {
        self.request_assign_handler.clone()
    }

    fn add_portal(&self, portal: Portal) {
        let portal_key = portal.info.portal_key.clone();
        self.portals.insert( portal_key.clone(), portal );
        self.portal_added_broadcast_tx.send( portal_key );
    }
}


fn test_logger(message: &str) {
    println!("{}", message);
}

pub struct StarlaneMeshRouter {
    skel: StarSkel,
}

#[derive(Debug)]
pub struct MechtronPortalRequestHandler {
    skel: StarSkel
}

impl MechtronPortalRequestHandler {
    pub fn new(skel: StarSkel)-> Self {
        MechtronPortalRequestHandler {
            skel
        }
    }
}

#[async_trait]
impl PortalRequestHandler for MechtronPortalRequestHandler {
    async fn route_to_mesh(&self, request: Request) -> Response {
        self.skel.messaging_api.exchange(request).await
    }
}
