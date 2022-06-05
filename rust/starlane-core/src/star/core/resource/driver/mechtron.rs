use std::collections::HashMap;
use std::future::Future;
use std::process::Child;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use anyhow::anyhow;
use dashmap::DashMap;
use dashmap::mapref::one::Ref;
use mesh_portal_api_server::{Portal, PortalApi, PortalEvent, PortalRequestHandler, PortalParticleApi};
use mesh_portal::version::latest::artifact::{ArtifactRequest, ArtifactResponse};

use crate::artifact::ArtifactRef;
use crate::error::Error;
use crate::particle::{ArtifactSubKind, KindBase, ParticleAssign, AssignParticleStateSrc, Kind, AssignKind};
use crate::star::core::resource::driver::ParticleCoreDriver;
use crate::star::core::resource::state::StateStore;
use crate::star::{StarSkel};
use crate::util::AsyncHashMap;
use crate::message::delivery::Delivery;
use mesh_portal::version::latest::command::common::StateSrc;
use mesh_portal::version::latest::config;
use mesh_portal::version::latest::id::Point;
use mesh_portal::version::latest::messaging::{Request, Response};
use mesh_portal::version::latest::portal;
use mesh_portal::version::latest::portal::Exchanger;
use mesh_portal::version::latest::portal::inlet::AssignRequest;
use mesh_portal::version::latest::particle::Properties;
use mesh_portal_tcp_server::{PortalServer, PortalTcpServer, TcpServerCall};
use tokio::sync::{broadcast, mpsc, oneshot, RwLock};
use tokio::sync::broadcast::Receiver;
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot::error::RecvError;
use mesh_portal::version::latest::config::{ParticleConfigBody, PointConfig};
use mesh_portal_versions::version::v0_0_1::particle::particle::ParticleDetails;

use crate::config::config::{MechtronConfig, ParticleConfig};

use crate::fail::Fail;
use crate::mechtron::process::launch_mechtron_process;
use crate::message::{Reply, StarlaneMessenger};
use crate::star::core::resource::driver::DriverCall::Assign;
use crate::starlane::api::StarlaneApi;


pub struct MechtronCoreDriver {
    skel: StarSkel,
    processes: HashMap<String, Child>,
    inner: Arc<RwLock<MechtronManagerInner>>,
    resource_type: KindBase,
    mechtron_portal_server_tx: Sender<TcpServerCall>,
}

impl MechtronCoreDriver {
    pub async fn new(skel: StarSkel, resource_type: KindBase) -> Result<Self,Error> {

        let mechtron_portal_server_tx = skel.machine.start_mechtron_portal_server().await?;

        let inner = Arc::new( RwLock::new(MechtronManagerInner::new() ) );

        {
            let mechtron_portal_server_tx = mechtron_portal_server_tx.clone();
            let inner = inner.clone();
            tokio::spawn( async move {
                match MechtronCoreDriver::get_portal_broadcast_tx(&mechtron_portal_server_tx).await {
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
                                PortalEvent::ParticleAdded(resource) => {

println!("Particle ADDDED: '{}'", resource.stub.point.to_string() );
                                    let point = resource.stub.point.clone();
                                    inner.mechtrons.insert( point.clone() , resource.clone() );
                                    if let Some(exchanges) = inner.mechtron_exchange.remove( &point ) {
                                        for tx in exchanges {
                                            tx.send( resource.clone() );
                                        }
                                    }
                                }
                                PortalEvent::ParticleRemoved(point) => {
                                    inner.mechtrons.remove(&point);
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

        Ok(MechtronCoreDriver {
            skel: skel.clone(),
            processes: HashMap::new(),
            inner,
            resource_type,
            mechtron_portal_server_tx,
        })
    }
}

impl MechtronCoreDriver {
   async fn get_portal_broadcast_tx(mechtron_portal_server_tx: &mpsc::Sender<TcpServerCall>) -> Result<broadcast::Receiver<PortalEvent>,Error> {
       let (tx, rx) = oneshot::channel();
       mechtron_portal_server_tx.send(TcpServerCall::GetPortalEvents(tx)).await;
       Ok(rx.await?)
   }
}

#[async_trait]
impl ParticleCoreDriver for MechtronCoreDriver {



    async fn assign(
        &mut self,
        assign: ParticleAssign,
    ) -> Result<(), Error> {
        match assign.state {
            StateSrc::None => {}
            _ => {
                return Err("currently only supporting stateless mechtrons".into());
            }
        };

println!("Assigning Mechtron...");

        let config_point = assign.config.properties.get(&"config".to_string() ).ok_or(format!("'config' property required to be set for {}", self.resource_type.to_string() ))?.value.as_str();
        let config_point = Point::from_str(config_point)?;

        let config_artifact_ref = ArtifactRef {
          point:config_point.clone(),
          kind: ArtifactSubKind::ParticleConfig
        };

        let caches = self.skel.machine.cache( &config_artifact_ref ).await?;

println!("MECHTRON: got caches" );
        let config = caches.resource_configs.get(&config_point).ok_or::<Error>(format!("expected mechtron_config").into())?;
println!("MECHTRON: got config" );
        let config = MechtronConfig::new(config, assign.config.stub.point.clone() );

println!("MechtronConfig.wasm_src().is_ok() {}", config.wasm_src().is_ok() );
println!("MechtronConfig.wasm_src() {}", config.wasm_src()?.to_string() );

        let api = StarlaneMessenger::new( self.skel.surface_api.clone() );
        let substitution_map = config.substitution_map()?;
        for command_line in &config.install {
//            let command_line = substitute(command_line.as_str(), &substitution_map)?;
            println!("INSTALL: '{}'",command_line);
            let mut output_rx = CommandExecutor::exec_simple(command_line.to_string(), assign.config.stub.clone(), api.clone() );
            while let Some(frame) = output_rx.recv().await {
                match frame {
                    outlet::Frame::StdOut(out) => {
                        println!("{}",out);
                    }
                    outlet::Frame::StdErr(out) => {
                        eprintln!("{}", out);
                    }
                    outlet::Frame::End(code) => {
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

        let portal_assign = ParticleAssign{
            kind: AssignKind::Create,
            config: PointConfig{
                body: ParticleConfigBody::Named(config.mechtron_name()?),
                point: assign.config.stub.point.clone()
            },
            state: StateSrc::None
        };

        portal.assign(portal_assign);

        Ok(())
    }

    async fn handle_request(&self, request: Request ) -> Response {

info!("handling request");

        let mechtron_rx = {
            let mut inner = self.inner.write().await;
            inner.exchange_mechtron(&request.to)
        };

        info!("found mechtron_rx");

        match mechtron_rx.await {
            Ok(mechtron) => {
                info!("got mechtron");
                mechtron.handle_request(request).await
            }
            Err(err) => {
                error!("{}",err.to_string());
                request.fail(err.to_string().as_str() )
            }
        }
    }


    fn kind(&self) -> KindBase {
        self.resource_type.clone()
    }
}

struct MechtronManagerInner{
    pub portals: HashMap<String,PortalApi>,
    pub mechtrons: HashMap<Point, PortalParticleApi>,
    pub portal_exchange: HashMap<String,Vec<oneshot::Sender<PortalApi>>>,
    pub mechtron_exchange: HashMap<Point,Vec<oneshot::Sender<PortalParticleApi>>>,
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

    pub fn exchange_mechtron(&mut self, point: &Point) -> oneshot::Receiver<PortalParticleApi> {
        let (tx,rx) = oneshot::channel();
        if let Some(mechtron) = self.mechtrons.get(point) {
            tx.send(mechtron.clone() );
            return rx;
        }

        let vec_tx: &mut Vec<oneshot::Sender<PortalParticleApi>> = match self.mechtron_exchange.get_mut(point ) {
            None => {
                self.mechtron_exchange.insert(point.clone(), vec![]);
                self.mechtron_exchange.get_mut( point).expect("expected vec")
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
            point: request.point.clone(),
            kind: ArtifactSubKind::Raw
        };
        let caches = self.api.cache( &artifact_ref).await?;
        let artifact = caches.raw.get(&request.point).ok_or(anyhow!("could not get raw artifact: '{}'",request.point.to_string()))?;
        Ok(ArtifactResponse{
            to: request.point.clone(),
            payload: artifact.data()
        })
    }

}
