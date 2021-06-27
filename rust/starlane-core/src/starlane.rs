use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::convert::TryInto;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::future::join_all;
use tokio::sync::mpsc;
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
use crate::lane::{ConnectionInfo, ConnectionKind, Lane, LocalTunnelConnector, ServerSideTunnelConnector, ClientSideTunnelConnector, ConnectorController };
use crate::logger::{Flags, Logger};
use crate::message::{Fail, ProtoStarMessage};
use crate::proto::{
    local_tunnels, ProtoStar, ProtoStarController, ProtoStarEvolution, ProtoTunnel,
};
use crate::resource::space::SpaceState;
use crate::resource::{
    AddressCreationSrc, AssignResourceStateSrc, KeyCreationSrc, ResourceAddress, ResourceArchetype,
    ResourceCreate, ResourceKind, ResourceRecord,
};
use crate::star::variant::{StarVariantFactory, StarVariantFactoryDefault};
use crate::star::{Request, Star, StarCommand, StarController, StarKey, StarName};
use crate::starlane::api::StarlaneApi;
use crate::template::{ConstellationData, ConstellationTemplate, StarKeyIndexTemplate, StarKeySubgraphTemplate, StarKeyTemplate, MachineName, ConstellationLayout};
use tokio::net::{TcpListener, TcpStream, TcpSocket};
use crate::util::AsyncHashMap;
use futures::{TryFutureExt, StreamExt};
use futures::stream::{SplitStream, SplitSink};
use std::net::{SocketAddr, SocketAddrV4, Ipv4Addr};
use std::str::FromStr;


pub mod api;

lazy_static! {
//    pub static ref DATA_DIR: Mutex<String> = Mutex::new("data".to_string());
    pub static ref DEFAULT_PORT: usize= 3719;
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

    pub async fn connect( &self, host: String, star_name: StarName) -> Result<ConnectorController,Error> {
        let (tx,rx) = oneshot::channel();
        self.tx.send( StarlaneCommand::Connect {host,star_name, tx }).await?;
        rx.await?
    }

    pub async fn create_constellation(&self, name: Option<String>, layout: ConstellationLayout ) -> Result<(),Error> {
        let (tx,rx) = oneshot::channel();
        let create = ConstellationCreate{
            name,
            layout,
            tx
        };
        self.tx.send( StarlaneCommand::ConstellationCreate(create)).await?;
        rx.await?
    }

    pub async fn star_control_request_by_name( &self, name: StarName ) -> Result<StarlaneApi,Error> {
        let (tx,rx) = oneshot::channel();
        let request = StarlaneApiRequestByName{
            name,
            tx
        };
        self.tx.send( StarlaneCommand::StarControlRequestByName(request)).await?;
        Ok(rx.await?)
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
    star_controllers: AsyncHashMap<StarKey, StarController>,
    star_names: AsyncHashMap<StarName, StarKey>,
    star_manager_factory: Arc<dyn StarVariantFactory>,
    //    star_core_ext_factory: Arc<dyn StarCoreExtFactory>,
    core_runner: Arc<CoreRunner>,
    data_access: FileAccess,
    cache_access: FileAccess,
    pub logger: Logger,
    pub flags: Flags,
    pub artifact_caches: Option<Arc<ProtoArtifactCachesFactory>>,
    port: usize,
    listening: bool
}

impl StarlaneMachineRunner {
    pub fn new(machine: String) -> Result<Self, Error> {
        let (tx, rx) = mpsc::channel(32);
        Ok(StarlaneMachineRunner {
            name: machine,
            star_controllers: AsyncHashMap::new(),
            star_names: AsyncHashMap::new(),
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
            listening: false
        })
    }

    pub async fn run(&mut self) {
        while let Option::Some(command) = self.rx.recv().await {
            match command {
                StarlaneCommand::Connect{ host, star_name, tx } => {
println!("connecting...to {}",host );
                    if let Ok(Option::Some(key)) = self.star_names.get(star_name.clone()).await {
println!("got starname...to {}",star_name.star);
                        if let Ok(Option::Some(ctrl)) = self.star_controllers.get(key).await {
println!("got controller...");
                            let result = self.add_client_side_lane_ctrl(ctrl.clone(), host ).await;
                            tx.send(result);
                        }
                    }
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
                StarlaneCommand::StarControlRequestByName(request) => {
                    if let Ok(Option::Some(key)) = self.star_names.get(request.name).await {
                        if let Ok(Option::Some(ctrl)) = self.star_controllers.get(key).await {
                            request.tx.send(StarlaneApi::new(ctrl.star_tx.clone()));
                        }
                    }
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
                    let star_name = StarName{
                        constellation: "standalone".to_string(),
                        star: "mesh".to_string()
                    };
                    if let Ok(Option::Some(key)) = self.star_names.get(star_name).await {
                        if let Ok(Option::Some(ctrl)) = self.star_controllers.get(key).await {
                            self.add_server_side_lane_ctrl(ctrl.clone(), stream).await;
                        }
                    }
                }
            }
        }
    }
    async fn constellation_create(
        &mut self,
        layout : ConstellationLayout,
        name: Option<String>,
    ) -> Result<(), Error> {

        println!("GOT HERE!");

        // create a list of all the machines we need to be able to create external/network lanes too
        let mut machines = vec![];
        for star_template in layout.template.stars.clone() {
            if let StarKeySubgraphTemplate::SubgraphKey(machine) = star_template.key.subgraph {
                machines.push(machine);
            }
        }

        for machine in machines {
            let host_address = match layout.machine_to_host_address.get(&machine) {
                None => format!("{}:{}", machine, (DEFAULT_PORT.clone() as usize).to_string()),
                Some(host_address) => host_address.clone()
            };



        }
        unimplemented!();

        /*
        let mut evolve_rxs = vec![];
        for star_template in layout.template.stars.clone() {


            println!("ABOUT TO CREATE ...!");
            let star_key = star_template.key.create(&data)?;
            println!("AND GOT HERE!");
            let (mut evolve_tx, mut evolve_rx) = oneshot::channel();
            evolve_rxs.push(evolve_rx);

            let (star_tx, star_rx) = mpsc::channel(32);
            if self.artifact_caches.is_none() {
                let api = StarlaneApi::new(star_tx.clone());
                let caches = Arc::new(ProtoArtifactCachesFactory::new(
                    api.into(),
                    self.cache_access.clone(),
                )?);
                self.artifact_caches = Option::Some(caches);
            }
            println!("EVEN HERE!");

            let (proto_star, star_ctrl) = ProtoStar::new(
                star_key.clone(),
                star_template.kind.clone(),
                star_tx,
                star_rx,
                self.artifact_caches
                    .as_ref()
                    .ok_or("already established that caches exists, what gives?")?
                    .clone(),
                self.data_access.clone(),
                self.star_manager_factory.clone(),
                self.core_runner.clone(),
                self.flags.clone(),
                self.logger.clone(),
            );

            let star_controllers = self.star_controllers.clone();
            let star_names = self.star_names.clone();
            let name = name.clone();
            println!("created proto star: {:?}", &star_template.kind);

            tokio::spawn(async move {
                let star = proto_star.evolve().await;
                if let Ok(star) = star {
                    let key = star.star_key().clone();

                    star_controllers
                        .put(key.clone(), star_ctrl.clone());
                    if name.is_some() && star_template.handle.is_some() {
                        let name = StarName {
                            constellation: name.as_ref().unwrap().clone(),
                            star: star_template.handle.as_ref().unwrap().clone(),
                        };
                        println!("inserting star {}.{}",name.constellation.clone(),name.star.clone());
                        star_names.put(name.clone(), key.clone()).await;
                    }

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
        }

        // now make the LANES
        for star_template in &template.stars {
            for lane in &star_template.lanes {
                let local = star_template.key.create(&data)?;
                let second = lane.star.create(&data)?;

                // since these are all local we should be able to unwrap
                self.add_local_lane(local.unwrap(), second.unwrap()).await?;
            }
        }

        // announce that the constellations is now complete
        for star_template in &template.stars {
            if let Ok(Option::Some(star_ctrl)) = self
                .star_controllers
                .get(star_template.key.create(&data)?.unwrap() ).await
            {
                star_ctrl
                    .star_tx
                    .send(StarCommand::ConstellationConstructionComplete)
                    .await;
            }
        }

        let evolutions = join_all(evolve_rxs).await;

        for evolve in evolutions {
            if let Ok(evolve) = evolve {
                evolve.controller.star_tx.send(StarCommand::Init).await;
                self.star_controllers.put(evolve.star, evolve.controller);
            } else if let Err(error) = evolve {
                return Err(error.to_string().into());
            }
        }

        Ok(())

         */
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

    async fn lookup_star_address(&self, key: &StarKey) -> Result<StarAddress, Error> {
        if self.star_controllers.contains(key.clone()).await? {
            Ok(StarAddress::Local)
        } else {
            Err(format!("could not find address for starkey: {}", key.to_string()).into())
        }
    }

    /*
    async fn provision_link(
        &mut self,
        template: ConstellationTemplate,
        mut data: ConstellationData,
        connection_info: ConnectionInfo,
    ) -> Result<(), Error> {
        let link = template.get_star("link".to_string());
        if link.is_none() {
            return Err("link is not present in the constellation template".into());
        }

        let link = link.unwrap().clone();
        let (mut evolve_tx, mut evolve_rx) = oneshot::channel();
        let (star_tx, star_rx) = mpsc::channel(32);

        if self.artifact_caches.is_none() {
            let api = StarlaneApi::new(star_tx.clone());
            let caches = Arc::new(ProtoArtifactCachesFactory::new(
                api.into(),
                self.cache_access.clone(),
            )?);
            self.artifact_caches = Option::Some(caches);
        }

        let (proto_star, star_ctrl) = ProtoStar::new(
            Option::None,
            link.kind.clone(),
            star_tx,
            star_rx,
            self.artifact_caches
                .clone()
                .ok_or("already established that caches exists, what gives?")?,
            self.data_access.clone(),
            self.star_manager_factory.clone(),
            self.core_runner.clone(),
            self.flags.clone(),
            self.logger.clone(),
        );

        println!("created proto star: {:?}", &link.kind);

        let starlane_ctrl = self.tx.clone();
        tokio::spawn(async move {
            let star = proto_star.evolve().await;
            if let Ok(star) = star {
                data.exclude_handles.insert("link".to_string());
                data.subgraphs
                    .insert("client".to_string(), star.star_key().subgraph.clone());

                let (tx, rx) = oneshot::channel();
                starlane_ctrl.send(StarlaneCommand::ConstellationCreate(ConstellationCreate {
                    name: Option::None,
                    layout: template,
                    data: data,
                    tx: tx,
                }));

                evolve_tx.send(ProtoStarEvolution {
                    star: star.star_key().clone(),
                    controller: StarController {
                        star_tx: star.star_tx(),
                    },
                });

                star.run().await;
            } else {
                eprintln!("experienced serious error could not evolve the proto_star");
            }
        });

        match connection_info.kind {
            ConnectionKind::Starlane => {
                let high_star_ctrl = star_ctrl.clone();
                let low_star_ctrl = {
                    let low_star_ctrl = self.star_controllers.get(connection_info.gateway.clone()).await?;
                    match low_star_ctrl {
                        None => {
                            return Err(format!(
                                "lane cannot construct. missing second star key: {}",
                                &connection_info.gateway.to_string()
                            )
                            .into())
                        }
                        Some(low_star_ctrl) => low_star_ctrl.clone(),
                    }
                };

                self.add_local_lane_ctrl(
                    Option::None,
                    Option::Some(connection_info.gateway.clone()),
                    high_star_ctrl,
                    low_star_ctrl,
                )
                .await?;
            }
            ConnectionKind::Url(_) => {
                eprintln!("not supported yet")
            }
        }

        if let Ok(evolve) = evolve_rx.await {
            self.star_controllers.put(evolve.star, evolve.controller).await;
        } else {
            eprintln!("got an error message on protostarevolution")
        }

        // now we need to create the lane to the desired gateway which is what the Link is all about

        Ok(())
    }
    */



    async fn add_local_lane(&mut self, local: StarKey, second: StarKey) -> Result<(), Error> {
        let (high, low) = StarKey::sort(local, second)?;
        let high_star_ctrl = {
            let high_star_ctrl = self.star_controllers.get(high.clone()).await?;
            match high_star_ctrl {
                None => {
                    return Err(format!(
                        "lane cannot construct. missing local star key: {}",
                        high.to_string()
                    )
                    .into())
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
                        low.to_string()
                    )
                    .into())
                }
                Some(low_star_ctrl) => low_star_ctrl.clone(),
            }
        };
        self.add_local_lane_ctrl(
            Option::Some(high),
            Option::Some(low),
            high_star_ctrl,
            low_star_ctrl,
        )
        .await
    }

    async fn add_local_lane_ctrl(
        &mut self,
        high: Option<StarKey>,
        low: Option<StarKey>,
        high_star_ctrl: StarController,
        low_star_ctrl: StarController,
    ) -> Result<(), Error> {
        let high_lane = Lane::new(low).await;
        let low_lane = Lane::new(high).await;
        let connector = LocalTunnelConnector::new(&high_lane, &low_lane).await?;
        high_star_ctrl
            .star_tx
            .send(StarCommand::AddLane(high_lane))
            .await?;
        low_star_ctrl
            .star_tx
            .send(StarCommand::AddLane(low_lane))
            .await?;
        high_star_ctrl
            .star_tx
            .send(StarCommand::AddConnectorController(connector))
            .await?;

        Ok(())
    }

    async fn add_server_side_lane_ctrl(
        &mut self,
        low_star_ctrl: StarController,
        stream: TcpStream
    ) -> Result<(), Error> {
        let low_lane = Lane::new(Option::None ).await;

        ServerSideTunnelConnector::new(&low_lane,stream).await?;

        low_star_ctrl
            .star_tx
            .send(StarCommand::AddLane(low_lane))
            .await?;

        Ok(())
    }


    async fn add_client_side_lane_ctrl(
        &mut self,
        star_ctrl: StarController,
        host: String
    ) -> Result<ConnectorController, Error> {
        let lane = Lane::new(Option::None ).await;

        let ctrl = ClientSideTunnelConnector::new(&lane,host).await?;

        star_ctrl
            .star_tx
            .send(StarCommand::AddLane(lane))
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
    Connect{ host: String, star_name: StarName, tx: oneshot::Sender<Result<ConnectorController,Error>>},
    ConstellationCreate(ConstellationCreate),
    StarControlRequestByKey(StarlaneApiRequestByKey),
    StarControlRequestByName(StarlaneApiRequestByName),
    Listen,
    AddStream(TcpStream),
    Destroy,
}

pub struct StarlaneApiRequestByKey {
    pub star: StarKey,
    pub tx: oneshot::Sender<StarlaneApi>,
}

pub struct StarlaneApiRequestByName {
    pub name: StarName,
    pub tx: oneshot::Sender<StarlaneApi>,
}

impl StarlaneApiRequestByName {
    pub fn new(constellation: String, star: String) -> (Self, oneshot::Receiver<StarlaneApi>) {
        let (tx, rx) = oneshot::channel();
        (
            StarlaneApiRequestByName {
                name: StarName {
                    constellation: constellation,
                    star: star,
                },
                tx: tx,
            },
            rx,
        )
    }
}

pub struct ConstellationCreate {
    name: Option<String>,
    layout: ConstellationLayout,
    tx: oneshot::Sender<Result<(), Error>>,
}


impl ConstellationCreate {
    pub fn new(
        layout: ConstellationLayout,
        name: Option<String>,
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
    use crate::starlane::{
        ConstellationCreate, StarlaneMachineRunner, StarlaneApiRequestByName, StarlaneCommand,
    };
    use crate::template::{ConstellationLayout, ConstellationTemplate};
    use std::convert::TryInto;
    use std::fs;
    use std::fs::File;
    use std::io::Read;

    #[test]
    pub fn starlane() {
        let data_dir = "tmp/data";
        let cache_dir = "tmp/cache";
        fs::remove_dir_all(data_dir).unwrap_or_default();
        fs::remove_dir_all(cache_dir).unwrap_or_default();
        std::env::set_var("STARLANE_DATA", data_dir);
        std::env::set_var("STARLANE_CACHE", cache_dir);

        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let mut starlane = StarlaneMachineRunner::new("server".to_string() ).unwrap();
            starlane.flags.on(Flag::Star(StarFlag::DiagnosePledge));
            let mut agg = LogAggregate::new();
            agg.watch(starlane.logger.clone()).await;
            let tx = starlane.tx.clone();

            let handle = tokio::spawn(async move {
                starlane.run().await;
            });

            {
                let (command, mut rx) = ConstellationCreate::new(
                    ConstellationLayout::standalone().unwrap(),
                    Option::Some("standalone".to_owned()),
                );
                tx.send(StarlaneCommand::ConstellationCreate(command)).await;
                let result = rx.await;
                match result {
                    Ok(result) => match result {
                        Ok(_) => {}
                        Err(e) => {
                            println!("error: {}", e)
                        }
                    },
                    Err(e) => {
                        println!("error: {}", e)
                    }
                }
            }

            tokio::time::sleep(Duration::from_secs(1)).await;

            let starlane_api = {
                let (request, rx) =
                    StarlaneApiRequestByName::new("standalone".to_owned(), "mesh".to_owned());
                tx.send(StarlaneCommand::StarControlRequestByName(request))
                    .await;
                timeout(Duration::from_millis(10), rx)
                    .await
                    .unwrap()
                    .unwrap()
            };

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
