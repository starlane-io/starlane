use std::collections::HashMap;
use std::future::Future;
use std::process::Child;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use anyhow::anyhow;
use dashmap::DashMap;
use dashmap::mapref::one::Ref;
use mesh_portal_api_server::{Portal, PortalApi, PortalEvent, PortalRequestHandler, PortalResourceApi};
use mesh_portal_serde::version::latest::artifact::{ArtifactRequest, ArtifactResponse};

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
use mesh_portal_serde::version::latest::messaging::{Request, Response};
use mesh_portal_serde::version::latest::payload::{Payload, PayloadPattern, Primitive};
use mesh_portal_serde::version::latest::portal;
use mesh_portal_serde::version::latest::portal::Exchanger;
use mesh_portal_serde::version::latest::portal::inlet::AssignRequest;
use mesh_portal_serde::version::latest::resource::Properties;
use mesh_portal_tcp_server::{PortalServer, PortalTcpServer, TcpServerCall};
use mesh_portal_versions::version::v0_0_1::config::ResourceConfigBody;
use mesh_portal_versions::version::v0_0_1::pattern::consume_data_struct_def;
use mesh_portal_versions::version::v0_0_1::util::ValueMatcher;
use tokio::sync::{broadcast, mpsc, oneshot, RwLock};
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


pub struct MechtronManager {
    skel: StarSkel,
    processes: HashMap<String, Child>,
    inner: Arc<RwLock<MechtronManagerInner>>,
    resource_type: ResourceType,
    mechtron_portal_server_tx: Sender<TcpServerCall>,
}

impl MechtronManager {
    pub async fn new(skel: StarSkel, resource_type:ResourceType) -> Result<Self,Error> {

        let mechtron_portal_server_tx = skel.machine.start_mechtron_portal_server().await?;

        let inner = Arc::new( RwLock::new(MechtronManagerInner::new() ) );

        {
            let mechtron_portal_server_tx = mechtron_portal_server_tx.clone();
            let inner = inner.clone();
            tokio::spawn( async move {
                match MechtronManager::get_portal_broadcast_tx(&mechtron_portal_server_tx).await {
                    Ok(mut portal_broadcast_rx) => {
                        while let Ok(event)  = portal_broadcast_rx.recv().await {
                            let mut inner = inner.write().await;
                            match event {
                                PortalEvent::PortalAdded(portal) => {
                                    inner.portals.insert(portal.info.portal_key.clone(),portal.clone() );
                                    if let Some(exchanges) = inner.portal_exchange.remove( &portal.info.portal_key) {
                                        for tx in exchanges {
                                            tx.send( portal.clone() );
                                        }
                                    }
                               }
                                PortalEvent::PortalRemoved(key) => {
                                    inner.portals.remove(&key);
                                }
                                PortalEvent::ResourceAdded(resource) => {
                                    let address = resource.stub.address.clone();
                                    inner.mechtrons.insert( address.clone() , resource.clone() );
                                    if let Some(exchanges) = inner.mechtron_exchange.remove( &address ) {
                                        for tx in exchanges {
                                            tx.send( resource.clone() );
                                        }
                                    }
                                }
                                PortalEvent::ResourceRemoved(address) => {
                                    inner.mechtrons.remove(&address);
                                }
                            }
                        }
                    }
                    Err(err) => {
                        error!("{}", err.to_string());
                    }
                }
            });
        }

        Ok(MechtronManager {
            skel: skel.clone(),
            processes: HashMap::new(),
            inner,
            resource_type,
            mechtron_portal_server_tx,
        })
    }
}

impl MechtronManager {
   async fn get_portal_broadcast_tx(mechtron_portal_server_tx: &mpsc::Sender<TcpServerCall>) -> Result<broadcast::Receiver<PortalEvent>,Error> {
       let (tx, rx) = oneshot::channel();
       mechtron_portal_server_tx.send(TcpServerCall::GetPortalEvents(tx)).await;
       Ok(rx.await?)
   }
}

#[async_trait]
impl ResourceManager for MechtronManager {



    async fn assign(
        &mut self,
        assign: ResourceAssign,
    ) -> Result<(), Error> {
        match assign.state {
            StateSrc::Stateless => {}
            _ => {
                return Err("currently only supporting stateless mechtrons".into());
            }
        };

println!("Assigning Mechtron...");

        let config_address = assign.stub.properties.get(&"config".to_string() ).ok_or(format!("'config' property required to be set for {}", self.resource_type.to_string() ))?.value.as_str();
        let config_address = Address::from_str(config_address)?;

        let config_artifact_ref = ArtifactRef {
          address:config_address.clone(),
          kind: ArtifactKind::ResourceConfig
        };

        let caches = self.skel.machine.cache( &config_artifact_ref ).await?;

println!("MECHTRON: got caches" );
        let config = caches.resource_configs.get(&config_address).ok_or::<Error>(format!("expected mechtron_config").into())?;
println!("MECHTRON: got config" );
        let config = MechtronConfig::new(config, assign.stub.address.clone() );

println!("MechtronConfig.wasm_src().is_ok() {}", config.wasm_src().is_ok() );
println!("MechtronConfig.wasm_src() {}", config.wasm_src()?.to_string() );

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
        let portal_key = wasm_src.to_string();

        let portal_rx = {
           let mut inner = self.inner.write().await;
           if !inner.portals.contains_key(&portal_key )  {
               self.processes.insert( portal_key.clone(), launch_mechtron_process(wasm_src.clone())? );
           }
           inner.exchange_portal(&portal_key)
        };

        let portal = portal_rx.await?;

        let portal_assign = Assign {
            config: Config {
                body: ResourceConfigBody::Named(config.mechtron_name()?),
                address: assign.stub.address.clone()
            },
            stub: assign.stub.clone()
        };
        portal.assign(portal_assign);

        Ok(())
    }

    async fn handle_request(&self, request: Request ) -> Response {

        let mechtron_rx = {
            let mut inner = self.inner.write().await;
            inner.exchange_mechtron(&request.to)
        };

        match mechtron_rx.await {
            Ok(mechtron) => {
                mechtron.handle_request(request).await
            }
            Err(err) => {
                request.fail(err.to_string().as_str() )
            }
        }
    }


    fn resource_type(&self) -> ResourceType {
        self.resource_type.clone()
    }
}

struct MechtronManagerInner{
    pub portals: HashMap<String,PortalApi>,
    pub mechtrons: HashMap<Address, PortalResourceApi>,
    pub portal_exchange: HashMap<String,Vec<oneshot::Sender<PortalApi>>>,
    pub mechtron_exchange: HashMap<Address,Vec<oneshot::Sender<PortalResourceApi>>>,
}

impl MechtronManagerInner {
    pub fn new() -> Self {
        Self {
            portals: Default::default(),
            mechtrons: Default::default(),
            portal_exchange: Default::default(),
            mechtron_exchange: Default::default()
        }
    }

    pub fn exchange_portal( &mut self, portal_key: &String ) -> oneshot::Receiver<PortalApi> {

        let (tx,rx) = oneshot::channel();

        if let Some(portal) = self.portals.get(portal_key) {
            tx.send(portal.clone());
            return rx;
        }

        let vec_tx: &mut Vec<oneshot::Sender<PortalApi>> = match self.portal_exchange.get_mut(portal_key ) {
            None => {
                self.portal_exchange.insert(portal_key.clone(), vec![]);
                self.portal_exchange.get_mut( portal_key ).expect("expected vec")
            }
            Some(vec_tx) => vec_tx
        };

        vec_tx.push(tx);

        rx
    }

    pub fn exchange_mechtron( &mut self, address: &Address ) -> oneshot::Receiver<PortalResourceApi> {
        let (tx,rx) = oneshot::channel();
        if let Some(mechtron) = self.mechtrons.get(address) {
            tx.send(mechtron.clone() );
            return rx;
        }

        let vec_tx: &mut Vec<oneshot::Sender<PortalResourceApi>> = match self.mechtron_exchange.get_mut(address ) {
            None => {
                self.mechtron_exchange.insert(address.clone(), vec![]);
                self.mechtron_exchange.get_mut( address).expect("expected vec")
            }
            Some(vec_tx) => vec_tx
        };

        vec_tx.push(tx);

        rx
    }
}

pub struct MechtronPortalServer {
    pub api: StarlaneApi,
    pub request_assign_handler: Arc<dyn PortalRequestHandler>,
    portals: Arc<DashMap<String, Portal>>,
}

impl MechtronPortalServer {
    pub fn new(api: StarlaneApi) -> Self {
        Self {
            api: api.clone(),
            request_assign_handler: Arc::new(MechtronPortalRequestHandler::new(api) ),
            portals: Arc::new(DashMap::new() ),
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
    }
}


fn test_logger(message: &str) {
    println!("{}", message);
}


pub struct MechtronPortalRequestHandler {
    api: StarlaneApi
}

impl MechtronPortalRequestHandler {
    pub fn new(api: StarlaneApi)-> Self {
        MechtronPortalRequestHandler {
            api
        }
    }
}

#[async_trait]
impl PortalRequestHandler for MechtronPortalRequestHandler {
    async fn route_to_mesh(&self, request: Request) -> Response {
        self.api.exchange(request).await
    }

    async fn handle_artifact_request(
        &self,
        request: ArtifactRequest,
    ) -> Result<ArtifactResponse, anyhow::Error> {

        let artifact_ref = ArtifactRef {
            address: request.address.clone(),
            kind: ArtifactKind::Raw
        };
        let caches = self.api.cache( &artifact_ref).await?;
        let artifact = caches.raw.get(&request.address).ok_or(anyhow!("could not get raw artifact: '{}'",request.address.to_string()))?;
        Ok(ArtifactResponse{
            to: request.address.clone(),
            payload: artifact.data()
        })
    }

}
