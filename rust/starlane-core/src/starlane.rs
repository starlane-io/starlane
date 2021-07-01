use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::convert::TryInto;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::future::{join_all, BoxFuture};
use tokio::sync::{mpsc, broadcast};
use tokio::sync::oneshot;
use tokio::sync::oneshot::error::RecvError;

use api::SpaceApi;
use serde::{Serialize,Deserialize};

use crate::cache::ProtoArtifactCachesFactory;
use crate::core::CoreRunner;
use crate::error::Error;
use crate::file_access::FileAccess;
use crate::frame::{ChildManagerResourceAction, Frame, Reply, SimpleReply, StarMessagePayload};
use crate::keys::ResourceKey;
use crate::lane::{ConnectionInfo, ConnectionKind, LaneEndpoint, LocalTunnelConnector, ServerSideTunnelConnector, ClientSideTunnelConnector, ConnectorController, ProtoLaneEndpoint, FrameCodex};
use crate::logger::{Flags, Logger};
use crate::message::{Fail, ProtoStarMessage};
use crate::star::{ConstellationBroadcast, StarKind, StarStatus};
use crate::proto::{
    local_tunnels, ProtoStar, ProtoStarController, ProtoStarEvolution, ProtoTunnel,
};
use crate::resource::space::SpaceState;
use crate::resource::{
    AddressCreationSrc, AssignResourceStateSrc, KeyCreationSrc, ResourceAddress, ResourceArchetype,
    ResourceCreate, ResourceKind, ResourceRecord,
};
use crate::star::variant::{StarVariantFactory, StarVariantFactoryDefault};
use crate::star::{Request, Star, StarCommand, StarController, StarKey, StarTemplateId, StarInfo};
use crate::starlane::api::StarlaneApi;
use crate::template::{ConstellationData, ConstellationTemplate, StarKeyConstellationIndexTemplate, StarKeySubgraphTemplate, StarKeyTemplate, MachineName, ConstellationLayout, StarSelector, ConstellationSelector, StarTemplate, StarInConstellationTemplateHandle, StarTemplateHandle, StarInConstellationTemplateSelector, ConstellationTemplateHandle};
use tokio::net::{TcpListener, TcpStream, TcpSocket};
use crate::util::AsyncHashMap;
use futures::{TryFutureExt, StreamExt, FutureExt};
use futures::stream::{SplitStream, SplitSink};
use std::net::{SocketAddr, SocketAddrV4, Ipv4Addr};
use std::str::FromStr;
use crate::constellation::{Constellation, ConstellationStatus};
use tokio::time::sleep;
use std::collections::hash_map::RandomState;
use std::cell::Cell;
use std::future::Future;

pub mod api;

lazy_static! {
//    pub static ref DATA_DIR: Mutex<String> = Mutex::new("data".to_string());
    pub static ref DEFAULT_PORT: usize = { std::env::var("STARLANE_PORT").unwrap_or("4343".to_string()).parse::<usize>().unwrap_or(4343) };

    pub static ref VERSION: VersionFrame = VersionFrame{ product: "Starlane".to_string(), version: "1.0.0".to_string() };
}

#[derive(Clone)]
pub struct StarlaneMachine {
    tx: mpsc::Sender<StarlaneCommand>,
    run_complete_signal_tx: broadcast::Sender<()>
}

impl StarlaneMachine {
    pub fn new(name: MachineName) -> Result<Self,Error> {
        Self::new_with_artifact_caches(name, Option::None)
    }

    pub fn new_with_artifact_caches(name: MachineName, artifact_caches: Option<Arc<ProtoArtifactCachesFactory>>) -> Result<Self,Error> {
        let mut runner = StarlaneMachineRunner::new_with_artifact_caches(name, artifact_caches )?;
        let tx = runner.command_tx.clone();
        let run_complete_signal_tx= runner.run();
        let starlane = Self {
            tx: tx,
            run_complete_signal_tx: run_complete_signal_tx
        };

        Ok(starlane)
    }

    pub async fn get_proto_artifact_caches_factory( &self ) -> Result<Option<Arc<ProtoArtifactCachesFactory>>,Error>
    {
        let (tx,rx) = oneshot::channel();
        self.tx.send( StarlaneCommand::GetProtoArtifactCachesFactory(tx)).await?;
        Ok(rx.await?)
    }

    pub fn shutdown(&self) {
        let tx = self.tx.clone();
        tokio::spawn( async move {
            tx.send(StarlaneCommand::Shutdown).await;
        });
    }


    pub async fn create_constellation(&self, name: &str, layout: ConstellationLayout ) -> Result<(),Error> {
        let name = name.to_string();
        let (tx,rx) = oneshot::channel();
        let create = ConstellationCreate{
            name,
            layout,
            tx
        };
        self.tx.send( StarlaneCommand::ConstellationCreate(create)).await?;
        rx.await?
    }

    pub async fn get_starlane_api(&self) -> Result<StarlaneApi,Error> {
        let (tx,rx) = oneshot::channel();
        self.tx.send( StarlaneCommand::StarlaneApiSelectBest(tx)).await?;
        rx.await?
    }

    pub async fn listen(&self) -> Result<(),Error>{
        let command_tx = self.tx.clone();
        let (tx,rx) = oneshot::channel();
        command_tx.send(StarlaneCommand::Listen(tx)).await;
        rx.await?
    }

    pub async fn join(self){
        let mut run_complete_signal_rx = self.run_complete_signal_tx.subscribe();
        join!(run_complete_signal_rx.recv());
    }
}

pub struct StarlaneMachineRunner {
    name: MachineName,
    pub command_tx: mpsc::Sender<StarlaneCommand>,
    command_rx: mpsc::Receiver<StarlaneCommand>,
    star_controllers: AsyncHashMap<StarInConstellationTemplateHandle, StarController>,
    star_manager_factory: Arc<dyn StarVariantFactory>,
    //    star_core_ext_factory: Arc<dyn StarCoreExtFactory>,
    core_runner: Arc<CoreRunner>,
    data_access: FileAccess,
    cache_access: FileAccess,
    pub logger: Logger,
    pub flags: Flags,
    pub artifact_caches: Option<Arc<ProtoArtifactCachesFactory>>,
    constellations: HashMap<String,Constellation>,
    port: usize,
    inner_flags: Arc<Mutex<Cell<StarlaneInnerFlags>>>,
}

impl StarlaneMachineRunner {
    pub fn new(machine: String) -> Result<Self, Error> {
        Self::new_with_artifact_caches(machine,Option::None)
    }

    pub fn new_with_artifact_caches(machine: String, artifact_caches: Option<Arc<ProtoArtifactCachesFactory>>) -> Result<Self, Error> {
        let (command_tx, command_rx) = mpsc::channel(32);
        Ok(StarlaneMachineRunner {
            name: machine,
            star_controllers: AsyncHashMap::new(),
            command_tx,
            command_rx,
            star_manager_factory: Arc::new(StarVariantFactoryDefault {}),
            //            star_core_ext_factory: Arc::new(ExampleStarCoreExtFactory::new() ),
            core_runner: Arc::new(CoreRunner::new()?),
            logger: Logger::new(),
            flags: Flags::new(),
            data_access: FileAccess::new(
                std::env::var("STARLANE_DATA").unwrap_or("data".to_string()),
            )?,
            cache_access: FileAccess::new(
                std::env::var("STARLANE_CACHE").unwrap_or("cache".to_string()),
            )?,
            artifact_caches: artifact_caches,
            port: DEFAULT_PORT.clone(),
            constellations: HashMap::new(),
            inner_flags: Arc::new(Mutex::new(Cell::new(StarlaneInnerFlags::new()) )),
        })
    }

    pub fn run(mut self) -> broadcast::Sender<()> {
        let (run_complete_signal_tx, _) = broadcast::channel(1);
        let run_complete_signal_tx_rtn = run_complete_signal_tx.clone();

        tokio::spawn( async move {
            while let Option::Some(command) = self.command_rx.recv().await {
                match command {

                    StarlaneCommand::ConstellationCreate(command) => {
                        let result = self
                            .constellation_create(command.layout, command.name)
                            .await;

//sleep(Duration::from_secs(10)).await;
                        if let Err(error) = &result {
                            error!("CONSTELLATION CREATE ERROR: {}", error.to_string());
                        }
                        command.tx.send(result);
                    }
                    StarlaneCommand::StarlaneApiSelectBest(tx) => {
                        let mut map = match self.star_controllers.clone().into_map().await {
                            Ok(map) => { map }
                            Err(err) => {
                                tx.send(Err(err));
                                continue;
                            }
                        };
                        if map.is_empty() {
                            tx.send(Err("ERROR: cannot create StarlaneApi: no StarControllers available.".into()));
                            continue;
                        }
                        let values: Vec<StarController> = map.into_iter().map(|(k, v)| v).collect();

                        let mut best = Option::None;

                        for star_ctrl in values {
                            let info = star_ctrl.get_star_info().await.unwrap().unwrap();
                            if best.is_none() {
                                best = Option::Some((info, star_ctrl));
                            } else {
                                let (prev_info, _) = best.as_ref().unwrap();
                                match info.kind {
                                    StarKind::Mesh => {
                                        best = Option::Some((info, star_ctrl));
                                    }
                                    StarKind::Client => {
                                        if prev_info.kind != StarKind::Mesh {
                                            best = Option::Some((info, star_ctrl));
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }

                        let (info, star_ctrl) = best.unwrap();

                        tx.send(Ok(StarlaneApi::new(star_ctrl.star_tx)));
                    }
                    StarlaneCommand::Shutdown => {
                        let mut inner_flags = self.inner_flags.lock().unwrap();
                        inner_flags.get_mut().shutdown = true;

                        self.command_rx.close();
                    }
                    StarlaneCommand::Listen(tx) => {
                        self.listen(tx);
                    }
                    StarlaneCommand::AddStream(stream) => {
                        match self.select_star_kind(&StarKind::Gateway).await {
                            Ok(Option::Some(star_ctrl)) => {
                                match self.add_server_side_lane_ctrl(star_ctrl, stream).await {
                                    Ok(result) => {}
                                    Err(error) => {
                                        error!("{}", error);
                                    }
                                }
                            }
                            Ok(Option::None) => {
                                error!("cannot find StarController for kind: StarKind::Gateway");
                            }
                            Err(err) => {
                                error!("{}", err);
                            }
                        }
                    }
                    StarlaneCommand::GetProtoArtifactCachesFactory(tx) => {
                        match self.artifact_caches.as_ref() {
                            None => {
                                tx.send(Option::None);
                            }
                            Some(caches) => {
                                tx.send(Option::Some(caches.clone()));
                            }
                        }
                    }
                }
            }
            run_complete_signal_tx.send(());
        });
        run_complete_signal_tx_rtn
    }

    async fn select_star_kind( &self, kind: &StarKind ) -> Result<Option<StarController>,Error> {
        let map = self.star_controllers.clone().into_map().await?;
        let values: Vec<StarController> = map.into_iter().map( |(k,v)| v ).collect();

        for star_ctrl in values {
            let info = star_ctrl.get_star_info().await?.ok_or("expected StarInfo")?;
            if info.kind == *kind {
                return Ok(Option::Some(star_ctrl));
            }
        }

        Ok(Option::None)
    }


    async fn constellation_create(
        &mut self,
        layout : ConstellationLayout,
        name: String,
    ) -> Result<(), Error> {
        if self.constellations.contains_key(&name) {
            return Err(format!("constellation named '{}' already exists in this StarlaneMachine.",name).into());
        }

        let mut constellation = Constellation::new(name.clone());
        let mut evolve_rxs = vec![];
        let (mut constellation_broadcaster,_) = broadcast::channel(16);

        for star_template in layout.template.stars.clone() {
            constellation.stars.push(star_template.clone());

            let star_template_id = StarInConstellationTemplateHandle::new( name.clone(), star_template.handle.clone() );

            let machine = layout.handles_to_machine.get(&star_template.handle  ).ok_or(format!("expected machine mapping for star template handle: {}",star_template.handle.to_string()))?;
            if self.name == *machine {
                let star_key = star_template.key.create();

                let (mut evolve_tx, mut evolve_rx) = oneshot::channel();
                evolve_rxs.push(evolve_rx);

                let (star_tx, star_rx) = mpsc::channel(32);

                let star_ctrl = StarController {
                    star_tx: star_tx.clone()
                };
                self.star_controllers.put(star_template_id, star_ctrl).await;

                if self.artifact_caches.is_none() {
                    let api = StarlaneApi::new(star_tx.clone());
                    let caches = Arc::new(ProtoArtifactCachesFactory::new(
                        api.into(),
                        self.cache_access.clone(),
                    )?);
                    self.artifact_caches = Option::Some(caches);
                }

                let (proto_star, star_ctrl) = ProtoStar::new(
                    star_key.clone(),
                    star_template.kind.clone(),
                    star_tx.clone(),
                    star_rx,
                    self.artifact_caches
                        .as_ref()
                        .ok_or("already established that caches exists, what gives?")?
                        .clone(),
                    self.data_access.clone(),
                    self.star_manager_factory.clone(),
                    self.core_runner.clone(),
                    constellation_broadcaster.subscribe(),
                    self.flags.clone(),
                    self.logger.clone(),
                );


                println!("created proto star: {:?}", &star_template.kind);

                tokio::spawn(async move {
                    let star = proto_star.evolve().await;
                    if let Ok(star) = star {
                        let key = star.star_key().clone();


                        let star_tx = star.star_tx();
                        tokio::spawn(async move {
                            star.run().await;
                        });
                        evolve_tx.send(ProtoStarEvolution {
                            star: key.clone(),
                            controller: StarController { star_tx: star_tx },
                        });
                        println!(
                            "created star: {:?} key: {}",
                            &star_template.kind,
                            &key.to_string()
                        );
                    } else {
                        eprintln!("experienced serious error could not evolve the proto_star");
                    }
                });
            } else {
                println!("skipping star not hosted on this machine: {}", star_template.handle.to_string() )
            }
        }

        // now connect the LANES
        let mut proto_lane_evolution_rxs = vec![];
        for star_template in &layout.template.stars {
            let machine = layout.handles_to_machine.get(&star_template.handle  ).ok_or(format!("expected machine mapping for star template handle: {}",star_template.handle.to_string()))?;
            let local_star = StarInConstellationTemplateHandle::new( name.clone(), star_template.handle.clone() );
println!("connecting for local: {}",local_star.star.to_string() );
            if self.name == *machine {
                for lane in &star_template.lanes {
                    match &lane.star_selector.constellation {
                                ConstellationSelector::Local => {
                                    let second_star = constellation.select( lane.star_selector.star.clone() ).ok_or("cannot select star from local constellation")?.clone();
                                    let second_star = StarInConstellationTemplateHandle::new( name.clone(), second_star.handle );
                                    let mut evolution_rxs = self.add_local_lane(local_star.clone(), second_star).await?;
                                    proto_lane_evolution_rxs.append( & mut evolution_rxs );
                                }
                                ConstellationSelector::Named(constellation_name) => {
                                    let constellation = self.constellations.get(constellation_name).ok_or(format!("cannot select constellation named '{}' on this StarlaneMachine",constellation_name))?;
                                    let second_star = constellation.select( lane.star_selector.star.clone() ).ok_or(format!("cannot select star from constellation {}",constellation_name))?.clone();
                                    let second_star = StarInConstellationTemplateHandle::new( constellation.name.clone(), second_star.handle );
                                    let mut evolution_rxs = self.add_local_lane(local_star.clone(), second_star).await?;
                                    proto_lane_evolution_rxs.append( & mut evolution_rxs );
                                }
                                ConstellationSelector::AnyWithGatewayInsideMachine(machine_name) => {
                                    let host_address = layout.get_machine_host_adddress(machine_name.clone());
                                    let star_ctrl = self.star_controllers.get(local_star.clone() ).await?.ok_or("expected local star to have star_ctrl")?;
                                    let proto_lane_evolution_rx= self.add_client_side_lane_ctrl(star_ctrl, host_address, lane.star_selector.clone(), true ).await?;
                                    proto_lane_evolution_rxs.push( proto_lane_evolution_rx );
                                }

                    }
                }
            }
        }



        let proto_lane_evolutions = join_all(proto_lane_evolution_rxs.iter_mut().map( |x| x.recv())).await;

        for result in proto_lane_evolutions {
            result??;
        }

        // announce that the local constellation is now complete
        constellation_broadcaster.send( ConstellationBroadcast::Status( ConstellationStatus::Assembled ));

        let evolutions = join_all(evolve_rxs).await;

        for evolve in evolutions {
            if let Ok(evolve) = evolve {
                evolve.controller.star_tx.send(StarCommand::Init).await;
            } else if let Err(error) = evolve {
                return Err(error.to_string().into());
            }
        }

        let mut ready_futures = vec![];
        for star_template in &layout.template.stars {
            let machine = layout.handles_to_machine.get(&star_template.handle).ok_or(format!("expected machine mapping for star template handle: {}", star_template.handle.to_string()))?;
            if self.name == *machine {
                let local_star = StarInConstellationTemplateHandle::new(name.clone(), star_template.handle.clone());
                let star_ctrl = self.star_controllers.get(local_star.clone()).await?.ok_or(format!("expected star controller: {}",local_star.to_string()) )?;
                let (tx,rx) = oneshot::channel();
                star_ctrl.star_tx.send(StarCommand::GetStatusListener(tx)).await;
                let mut star_status_receiver = rx.await?;
                let (ready_status_tx,ready_status_rx) = oneshot::channel();
                tokio::spawn( async move {
                   while let Result::Ok( status) = star_status_receiver.recv().await {
                       if status == StarStatus::Ready {
                           ready_status_tx.send(());
                           break;
                       }
                   }
                });
                ready_futures.push(ready_status_rx);
            }
        }

        // wait for all stars to be StarStatus::Ready
        join_all(ready_futures ).await;

        Ok(())
    }

    fn listen(&mut self, result_tx: oneshot::Sender<Result<(),Error>>){
        {
            let mut inner_flags = self.inner_flags.lock().unwrap();
            let flags = inner_flags.get_mut();

            if flags.listening {
                result_tx.send(Ok(()));
                return;
            }
            flags.listening = true;
        }


        {
            let port = self.port.clone();
            let inner_flags = self.inner_flags.clone();

/*            ctrlc::set_handler( move || {
                Self::unlisten(inner_flags.clone(), port.clone());
            }).expect("expected to be able to set ctrl-c handler");
 */

        }

        let port = self.port.clone();
        let command_tx = self.command_tx.clone();
        let flags = self.inner_flags.clone();
        tokio::spawn( async move {
            match std::net::TcpListener::bind(format!("127.0.0.1:{}",port)){
                Ok(std_listener) => {
                    let listener = TcpListener::from_std(std_listener).unwrap();
                    result_tx.send(Ok(()) );
                    while let Ok((mut stream,_)) = listener.accept().await {
                        {
                            let mut flags = flags.lock().unwrap();
                            let flags = flags.get_mut();
                            if flags.shutdown {
                                drop(listener);
                                return;
                            }
                        }
info!("client connection made");
                        let ok = command_tx.send( StarlaneCommand::AddStream(stream) ).await.is_ok();
                        tokio::time::sleep(Duration::from_secs(0)).await;
                    }
                }
                Err(error) => {
                    error!("FATAL: could not setup TcpListener {}", error);
                    result_tx.send(Err(error.into()) );
                }
            }
        } );
    }



    pub fn caches(&self) -> Result<Arc<ProtoArtifactCachesFactory>, Error> {
        Ok(self
            .artifact_caches
            .as_ref()
            .ok_or("expected caches to be set")?
            .clone())
    }

    async fn add_local_lane(&mut self, local: StarInConstellationTemplateHandle, second: StarInConstellationTemplateHandle ) -> Result<Vec<broadcast::Receiver<Result<(),Error>>>, Error> {
        let (high, low) = crate::util::sort(local, second)?;

        let high_star_ctrl = {
            let high_star_ctrl = self.star_controllers.get(high.clone()).await?;
            match high_star_ctrl {
                None => {
                    return Err(format!(
                        "lane cannot construct. missing local star key: {}",
                        high.star.to_string()
                    )
                    .into());
                }
                Some(high_star_ctrl) => high_star_ctrl.clone(),
            }
        };

        let low_star_ctrl = {
            let low_star_ctrl = self.star_controllers.get(low.clone()).await?;
            match low_star_ctrl {
                None => {
                    return Err(format!(
                        "lane cannot construct. missing second star key: {}",
                        low.star.to_string()
                    )
                    .into());
                }
                Some(low_star_ctrl) => low_star_ctrl.clone(),
            }
        };
        self.add_local_lane_ctrl(
            high_star_ctrl,
            low_star_ctrl,
        )
        .await
    }

    async fn add_local_lane_ctrl(
        &mut self,
        high_star_ctrl: StarController,
        low_star_ctrl: StarController,
    ) -> Result<Vec<broadcast::Receiver<Result<(),Error>>>, Error> {
        let high_lane = ProtoLaneEndpoint::new(Option::None);
        let low_lane = ProtoLaneEndpoint::new(Option::None);
        let rtn = vec![high_lane.get_evoltion_rx(),low_lane.get_evoltion_rx()];
        let connector = LocalTunnelConnector::new(&high_lane, &low_lane).await?;
        high_star_ctrl
            .star_tx
            .send(StarCommand::AddProtoLaneEndpoint(high_lane))
            .await?;
        low_star_ctrl
            .star_tx
            .send(StarCommand::AddProtoLaneEndpoint(low_lane))
            .await?;
        high_star_ctrl
            .star_tx
            .send(StarCommand::AddConnectorController(connector))
            .await?;

        Ok(rtn)
    }

    async fn add_server_side_lane_ctrl(
        &mut self,
        low_star_ctrl: StarController,
        stream: TcpStream
    ) -> Result<broadcast::Receiver<Result<(),Error>>, Error> {
        let low_lane = ProtoLaneEndpoint::new(Option::None );
        let rtn = low_lane.get_evoltion_rx();

        let connector_ctrl = ServerSideTunnelConnector::new(&low_lane,stream).await?;

        low_star_ctrl
            .star_tx
            .send(StarCommand::AddProtoLaneEndpoint(low_lane))
            .await?;

        low_star_ctrl
            .star_tx
            .send(StarCommand::AddConnectorController(connector_ctrl))
            .await?;

        Ok(rtn)
    }

    async fn add_client_side_lane_ctrl(
        &mut self,
        star_ctrl: StarController,
        host_address: String,
        selector: StarInConstellationTemplateSelector,
        key_requestor: bool
        ) -> Result<broadcast::Receiver<Result<(),Error>>, Error> {

        let mut lane = ProtoLaneEndpoint::new(Option::None );
        lane.key_requestor = key_requestor;

        let rtn = lane.get_evoltion_rx();

        let connector= ClientSideTunnelConnector::new(&lane,host_address,selector).await?;

        star_ctrl
            .star_tx
            .send(StarCommand::AddProtoLaneEndpoint(lane))
            .await?;

        star_ctrl
            .star_tx
            .send(StarCommand::AddConnectorController(connector))
            .await?;

       Ok(rtn)
    }

    fn unlisten(inner_flags: Arc<Mutex<Cell<StarlaneInnerFlags>>>, port: usize ){
        {
            let mut flags = inner_flags.lock().unwrap();
            flags.get_mut().shutdown = true;
        }
        std::net::TcpStream::connect(format!("localhost:{}",port) );
        std::thread::sleep(std::time::Duration::from_secs(1) );
    }

}

impl Drop for StarlaneMachineRunner {
    fn drop(&mut self) {
        {
            let mut flags =self.inner_flags.lock().unwrap();

            let flags_mut = flags.get_mut();

            if !flags_mut.shutdown
            {
                warn!("dropping Starlane( {} ) unexpectedly", self.name);
            }

            if !flags_mut.listening {
                Self::unlisten(self.inner_flags.clone(), self.port.clone());
            }
        }
    }
}

#[derive(Clone,Serialize,Deserialize)]
pub struct VersionFrame {
    product: String,
    version: String
}

#[derive(strum_macros::Display)]
pub enum StarlaneCommand {
    ConstellationCreate(ConstellationCreate),
    StarlaneApiSelectBest(oneshot::Sender<Result<StarlaneApi,Error>>),
    Listen(oneshot::Sender<Result<(),Error>>),
    AddStream(TcpStream),
    GetProtoArtifactCachesFactory(oneshot::Sender<Option<Arc<ProtoArtifactCachesFactory>>>),
    Shutdown
}

pub struct StarlaneApiRequestByKey {
    pub star: StarKey,
    pub tx: oneshot::Sender<StarlaneApi>,
}

pub struct StarlaneApiRequest {
    pub selector: StarSelector,
    pub tx: oneshot::Sender<StarlaneApi>,
}

impl StarlaneApiRequest {
    pub fn new(selector: StarSelector) -> (Self, oneshot::Receiver<StarlaneApi>) {
        let (tx, rx) = oneshot::channel();
        (
            Self{
                selector: selector,
                tx: tx,
            },
            rx,
        )
    }
}

pub struct ConstellationCreate {
    name: String,
    layout: ConstellationLayout,
    tx: oneshot::Sender<Result<(), Error>>,
}


impl ConstellationCreate {
    pub fn new(
        layout: ConstellationLayout,
        name: String,
    ) -> (Self, oneshot::Receiver<Result<(), Error>>) {
        let (tx, rx) = oneshot::channel();
        (
            ConstellationCreate {
                name: name,
                layout: layout,
                tx: tx,
            },
            rx,
        )
    }
}

pub enum StarAddress {
    Local,
}

#[derive(Clone)]
struct  StarlaneInnerFlags {
    pub shutdown: bool,
    pub listening: bool
}

impl StarlaneInnerFlags{
    pub fn new()->Self{
        Self{
            shutdown: false,
            listening: false
        }
    }
}

#[cfg(test)]
mod test {
    use std::str::FromStr;
    use std::sync::Arc;

    use tokio::runtime::Runtime;
    use tokio::sync::oneshot::error::RecvError;
    use tokio::time::timeout;
    use tokio::time::Duration;

    use crate::artifact::{ArtifactAddress, ArtifactKind, ArtifactLocation, ArtifactBundleAddress};
    use crate::error::Error;
    use crate::keys::{SpaceKey, SubSpaceKey, UserKey};
    use crate::logger::{
        Flag, Flags, Log, LogAggregate, ProtoStarLog, ProtoStarLogPayload, StarFlag, StarLog,
        StarLogPayload,
    };
    use crate::message::Fail;
    use crate::names::Name;
    use crate::permissions::Authentication;
    use crate::resource::{Labels, ResourceAddress};
    use crate::space::CreateAppControllerFail;
    use crate::star::{StarController, StarInfo, StarKey, StarKind};
    use crate::starlane::api::SubSpaceApi;
    use crate::starlane::{ConstellationCreate, StarlaneMachineRunner, StarlaneApiRequest, StarlaneCommand, StarlaneMachine};
    use crate::template::{ConstellationLayout, ConstellationTemplate};
    use std::convert::TryInto;
    use std::fs;
    use std::fs::File;
    use std::io::Read;
    use tokio::sync::oneshot;
    use tracing::dispatcher::set_global_default;
    use tracing_subscriber::FmtSubscriber;

    #[test]
    #[instrument]
    pub fn tracing()
    {
        let subscriber = FmtSubscriber::default();
        set_global_default(subscriber.into()).expect("setting global default failed");
        info!("tracing works!");
    }

    #[test]
    pub fn starlane() {
        let subscriber = FmtSubscriber::default();
        set_global_default(subscriber.into()).expect("setting global default failed");
        info!("tracing works!");


        let data_dir = "tmp/data";
        let cache_dir = "tmp/cache";
        fs::remove_dir_all(data_dir).unwrap_or_default();
        fs::remove_dir_all(cache_dir).unwrap_or_default();
        std::env::set_var("STARLANE_DATA", data_dir);
        std::env::set_var("STARLANE_CACHE", cache_dir);

        let rt = Runtime::new().unwrap();
        rt.block_on(async {

            info!("entered block on!");

            let mut starlane = StarlaneMachine::new("server".to_string() ).unwrap();
            starlane.listen().await.unwrap();

            starlane.create_constellation("standalone", ConstellationLayout::standalone().unwrap() ).await.unwrap();

            let mut client = StarlaneMachine::new_with_artifact_caches("client".to_string(), starlane.get_proto_artifact_caches_factory().await.unwrap() ).unwrap();
            let mut client_layout = ConstellationLayout::client("gateway".to_string() ).unwrap();
            client_layout.set_machine_host_address("gateway".to_lowercase(), format!("localhost:{}", crate::starlane::DEFAULT_PORT.clone()));
            client.create_constellation("client", client_layout  ).await.unwrap();

            tokio::time::sleep(Duration::from_secs(1)).await;

            let starlane_api = client.get_starlane_api().await.unwrap();

            tokio::spawn( async move {
                println!("ping gateway...");
            });


            if starlane_api.ping_gateway().await.is_err() {
                error!("failed to ping gateway");
                client.shutdown();
                starlane.shutdown();
                return;
            }

            tokio::spawn( async move {
                println!("getting subspace_api....");
            });

            let sub_space_api = match starlane_api
                .get_sub_space(
                    ResourceAddress::from_str("hyperspace:default::<SubSpace>")
                        .unwrap()
                        .into(),
                )
                .await
            {
                Ok(api) => api,
                Err(err) => {
                    eprintln!("{}", err.to_string());
                    panic!(err)
                }
            };

            tokio::spawn( async move {
                println!("on to filesystem api ...");
            });

            let file_api = sub_space_api
                .create_file_system("website")
                .unwrap()
                .submit()
                .await
                .unwrap();
            file_api
                .create_file_from_string(
                    &"/index.html".try_into().unwrap(),
                    "The rain in Spain falls mostly on the plain.".to_string(),
                )
                .unwrap()
                .submit()
                .await
                .unwrap();
            file_api
                .create_file_from_string(
                    &"/second/index.html".try_into().unwrap(),
                    "This is a second page....".to_string(),
                )
                .unwrap()
                .submit()
                .await
                .unwrap();

            /*
            // upload an artifact bundle
            {
                let mut file =
                    File::open("test-data/localhost-config/artifact-bundle.zip").unwrap();
                let mut data = vec![];
                file.read_to_end(&mut data).unwrap();
                let data = Arc::new(data);
                let artifact_bundle_api = starlane_api
                    .create_artifact_bundle(
                        &ArtifactBundleAddress::from_str("hyperspace:default:whiz:1.0.0").unwrap(),
                        data,
                    ).await
                    .unwrap()
                    .submit()
                    .await
                    .unwrap();
            }
             */

            // upload an artifact bundle
            {
                let mut file =
                    File::open("test-data/localhost-config/artifact-bundle.zip").unwrap();
                let mut data = vec![];
                file.read_to_end(&mut data).unwrap();
                let data = Arc::new(data);
                let artifact_bundle_api = sub_space_api
                    .create_artifact_bundle(
                        "filo",
                        &semver::Version::from_str("1.0.0").unwrap(),
                        data,
                    )
                    .unwrap()
                    .submit()
                    .await
                    .unwrap();
            }

            tokio::spawn(async move {
              info!("done");
            });

            std::thread::sleep(std::time::Duration::from_secs(5) );

            client.shutdown();
            starlane.shutdown();

            std::thread::sleep(std::time::Duration::from_secs(1) );
        });
    }
}
