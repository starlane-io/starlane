use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::convert::TryInto;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::future::join_all;
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
use crate::lane::{ConnectionInfo, ConnectionKind, LaneEndpoint, LocalTunnelConnector, ServerSideTunnelConnector, ClientSideTunnelConnector, ConnectorController, ProtoLaneEndpoint};
use crate::logger::{Flags, Logger};
use crate::message::{Fail, ProtoStarMessage};
use crate::star::ConstellationBroadcast;
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
use crate::template::{ConstellationData, ConstellationTemplate, StarKeyIndexTemplate, StarKeySubgraphTemplate, StarKeyTemplate, MachineName, ConstellationLayout, StarSelector, ConstellationSelector, StarTemplate, StarInConstellationTemplateHandle, StarTemplateHandle, StarInConstellationTemplateSelector, ConstellationTemplateHandle};
use tokio::net::{TcpListener, TcpStream, TcpSocket};
use crate::util::AsyncHashMap;
use futures::{TryFutureExt, StreamExt};
use futures::stream::{SplitStream, SplitSink};
use std::net::{SocketAddr, SocketAddrV4, Ipv4Addr};
use std::str::FromStr;
use crate::constellation::Constellation;

pub mod api;

lazy_static! {
//    pub static ref DATA_DIR: Mutex<String> = Mutex::new("data".to_string());
    pub static ref DEFAULT_PORT: usize = 4343;
    pub static ref VERSION: VersionFrame = VersionFrame{ product: "Starlane".to_string(), version: "1.0.0".to_string() };
}

#[derive(Clone)]
pub struct StarlaneMachine {
    tx: mpsc::Sender<StarlaneCommand>
}

impl StarlaneMachine {
    pub fn new(name: MachineName) -> Result<Self,Error> {
        let mut runner = StarlaneMachineRunner::new(name)?;
        let starlane = Self {
            tx: runner.tx.clone()
        };

        tokio::spawn( async move {
            runner.run().await;
        });

        Ok(starlane)
    }

    pub async fn connect(&self, host: String, star_name: StarTemplateId) -> Result<ConnectorController,Error> {
        let (tx,rx) = oneshot::channel();
        self.tx.send( StarlaneCommand::Connect {host,star_name, tx }).await?;
        rx.await?
    }

    pub async fn create_constellation(&self, name: String, layout: ConstellationLayout ) -> Result<(),Error> {
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
        self.tx.send( StarlaneCommand::StarlaneApiSelectAny(tx)).await?;
        rx.await?
    }

    pub fn listen(&self) {
        let tx = self.tx.clone();
        tokio::spawn( async move {
            tx.send(StarlaneCommand::Listen).await;
        });
    }
}

pub struct StarlaneMachineRunner {
    name: MachineName,
    pub tx: mpsc::Sender<StarlaneCommand>,
    rx: mpsc::Receiver<StarlaneCommand>,
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
    listening: bool
}

impl StarlaneMachineRunner {
    pub fn new(machine: String) -> Result<Self, Error> {
        let (tx, rx) = mpsc::channel(32);
        Ok(StarlaneMachineRunner {
            name: machine,
            star_controllers: AsyncHashMap::new(),
            tx: tx,
            rx: rx,
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
            artifact_caches: Option::None,
            port: DEFAULT_PORT.clone(),
            listening: false,
            constellations: HashMap::new()
        })
    }

    pub async fn run(&mut self) {
        while let Option::Some(command) = self.rx.recv().await {
            match command {
                StarlaneCommand::Connect{ host, star_name, tx } => {
unimplemented!();
                }
                StarlaneCommand::ConstellationCreate(command) => {
println!("constellation CREATE");
                    let result = self
                        .constellation_create(command.layout, command.name)
                        .await;
if let Err(error) = &result {
    eprintln!("{}",error.to_string());
}
                    command.tx.send(result);
                }
                StarlaneCommand::StarlaneApiSelectAny(tx) => {
                    for (_,star_ctrl) in self.star_controllers.clone().into_map().await.unwrap_or(HashMap::new()) {
                        tx.send(Ok(StarlaneApi::new(star_ctrl.star_tx)));
                        return;
                    }
                    tx.send(Err("ERROR: cannot create StarlaneApi: no StarControllers available.".into()));
                }
                StarlaneCommand::Destroy => {
                    println!("closing rx");
                    self.rx.close();
                }
                StarlaneCommand::StarControlRequestByKey(_) => {
                    unimplemented!()
                }
                StarlaneCommand::Listen => {
                    self.listen();
                }
                StarlaneCommand::AddStream(stream)=> {
                    /*
                    let star_name = StarTemplateId {
                        constellation: "standalone".to_string(),
                        handle: "mesh".into()
                    };
                    if let Ok(Option::Some(key)) = self.star_names.get(star_name).await {
                        if let Ok(Option::Some(ctrl)) = self.star_controllers.get(key).await {
                            self.add_server_side_lane_ctrl(ctrl.clone(), stream).await;
                        }
                    }
                     */
                    unimplemented!("ADD STREAM IS NOT IMPLEMENTED")
                }
            }
        }
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
                    match lane {
                        StarSelector::StarInConstellationTemplate(constellation_selector) => {
                            match &constellation_selector.constellation{
                                ConstellationSelector::Local => {
                                    let second_star = constellation.select( constellation_selector.star.clone() ).ok_or("cannot select star from local constellation")?.clone();
                                    let second_star = StarInConstellationTemplateHandle::new( name.clone(), second_star.handle );
                                    let mut evolution_rxs = self.add_local_lane(local_star.clone(), second_star).await?;
                                    proto_lane_evolution_rxs.append( & mut evolution_rxs );
                                }
                                ConstellationSelector::Named(constellation_name) => {
                                    let constellation = self.constellations.get(constellation_name).ok_or(format!("cannot select constellation named '{}' on this StarlaneMachine",constellation_name))?;
                                    let second_star = constellation.select( constellation_selector.star.clone() ).ok_or(format!("cannot select star from constellation {}",constellation_name))?.clone();
                                    let second_star = StarInConstellationTemplateHandle::new( constellation.name.clone(), second_star.handle );
                                    let mut evolution_rxs = self.add_local_lane(local_star.clone(), second_star).await?;
                                    proto_lane_evolution_rxs.append( & mut evolution_rxs );
                                }
                                ConstellationSelector::AnyInsideMachine(machine_name) => {
                                    let host_address = layout.machine_host_address(machine_name.clone());
                                    let star_ctrl = self.star_controllers.get(local_star.clone() ).await?.ok_or("expected local star to have star_ctrl")?;
                                    self.add_client_side_lane_ctrl(star_ctrl, host_address, constellation_selector.clone() ).await?;
                                    unimplemented!()
                                }
                            }
                        }
                        _ => {
                            return Err("create constellation can only work with Template StarSelectors.".into());
                        }
                    }
                }
            }
        }

        let proto_lane_evolutions = join_all(proto_lane_evolution_rxs.iter_mut().map( |x| x.recv())).await;

        for result in proto_lane_evolutions {
            result??;
        }

println!("sending ConstellationReady signal");
        // announce that the local constellation is now complete
        constellation_broadcaster.send( ConstellationBroadcast::ConstellationReady );

        let evolutions = join_all(evolve_rxs).await;

        for evolve in evolutions {
            if let Ok(evolve) = evolve {
                evolve.controller.star_tx.send(StarCommand::Init).await;
            } else if let Err(error) = evolve {
                return Err(error.to_string().into());
            }
        }

        Ok(())

    }

    fn listen(&mut self) {
        if self.listening {
            return;
        }

        self.listening = true;
        let port = self.port.clone();
        let tx = self.tx.clone();
        tokio::spawn( async move {

            let std_listener = std::net::TcpListener::bind(format!("127.0.0.1:{}",port)).unwrap();
            let listener = TcpListener::from_std(std_listener).unwrap();
println!("LISTENING!");
            while let Ok((mut stream,_)) = listener.accept().await {
println!("new client!");
                tx.send( StarlaneCommand::AddStream(stream) ).await;
            }
            eprintln!("TCP LISTENER TERMINATED");
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

        ServerSideTunnelConnector::new(&low_lane,stream).await?;

        low_star_ctrl
            .star_tx
            .send(StarCommand::AddProtoLaneEndpoint(low_lane))
            .await?;

        Ok(rtn)
    }

    async fn add_client_side_lane_ctrl(
        &mut self,
        star_ctrl: StarController,
        host_address: String,
        selector: StarInConstellationTemplateSelector
    ) -> Result<ConnectorController, Error> {
        let lane = ProtoLaneEndpoint::new(Option::None );

        let ctrl = ClientSideTunnelConnector::new(&lane,host_address,selector).await?;

        star_ctrl
            .star_tx
            .send(StarCommand::AddProtoLaneEndpoint(lane))
            .await?;

        Ok(ctrl)
    }

}

#[derive(Clone,Serialize,Deserialize)]
pub struct VersionFrame {
    product: String,
    version: String
}

pub enum StarlaneCommand {
    Connect{ host: String, star_name: StarTemplateId, tx: oneshot::Sender<Result<ConnectorController,Error>>},
    ConstellationCreate(ConstellationCreate),
    StarControlRequestByKey(StarlaneApiRequestByKey),
    StarlaneApiSelectAny(oneshot::Sender<Result<StarlaneApi,Error>>),
    Listen,
    AddStream(TcpStream),
    Destroy,
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

#[cfg(test)]
mod test {
    use std::str::FromStr;
    use std::sync::Arc;

    use tokio::runtime::Runtime;
    use tokio::sync::oneshot::error::RecvError;
    use tokio::time::timeout;
    use tokio::time::Duration;

    use crate::artifact::{ArtifactAddress, ArtifactKind, ArtifactLocation};
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
            starlane.create_constellation("standalone".to_string(), ConstellationLayout::standalone().unwrap() ).await.unwrap();

            tokio::time::sleep(Duration::from_secs(1)).await;

            let starlane_api = starlane.get_starlane_api().await.unwrap();

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
            tokio::time::sleep(Duration::from_secs(1)).await;

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

            tokio::spawn( async {
                println!("... >  filesystems created ...");
            });

            // upload an artifact bundle
            {
                let mut file =
                    File::open("test-data/localhost-config/artifact-bundle.zip").unwrap();
                let mut data = vec![];
                file.read_to_end(&mut data).unwrap();
                let data = Arc::new(data);
                tokio::spawn( async {
                    println!("... >  uploading artifact bundle...");
                });
                let artifact_bundle_api = sub_space_api
                    .create_artifact_bundle(
                        "whiz",
                        &semver::Version::from_str("1.0.0").unwrap(),
                        data,
                    )
                    .unwrap()
                    .submit()
                    .await
                    .unwrap();
            }

            tokio::spawn( async {
                println!("... >  artifact bundle uploaded...");
            });

            /*
                        {
                            let artifact = ArtifactAddress::from_str("hyperspace:default:whiz:1.0.0:/routes.txt").unwrap();
                            let caches = starlane_api.get_caches().await.unwrap();
                            let domain_configs = caches.domain_configs.create();
                            domain_configs.wait_for_cache(artifact.clone() ).await.unwrap();
                            let domain_configs = domain_configs.into_cache().await.unwrap();
            println!("cache Ok!");
                            let domain_config = domain_configs.get(&artifact).unwrap();
            println!("got domain_config!");
                        }
                         */

//            loop {
                tokio::time::sleep(Duration::from_secs(1)).await;
 //           }

            //            assert_eq!(central_ctrl.diagnose_handlers_satisfaction().await.unwrap(),crate::star::pledge::Satisfaction::Ok)
        });
    }
}
