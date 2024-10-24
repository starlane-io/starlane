use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::sync::mpsc::SendError;
use std::sync::Arc;
use std::time::Duration;

use crate::driver::DriverErr;
use crate::err::{err, HypErr, HyperErr2};
use crate::hyperlane::{
    HyperClient, HyperConnectionDetails, HyperGate, HyperGateSelector, Hyperway, HyperwayEndpoint,
    HyperwayEndpointFactory, HyperwayInterchange, LayerTransform, MountInterchangeGate,
    SimpleGreeter,
};
use crate::hyperspace::reg::Registry;
use crate::hyperspace::star::{
    HyperStar, HyperStarApi, HyperStarSkel, HyperStarTx, StarCon, StarTemplate,
};
use crate::platform::Platform;
use crate::service::{
    service_conf, Service, ServiceConf, ServiceErr, ServiceKind, ServiceSelector, ServiceTemplate,
};
use crate::template::Templates;
use dashmap::DashMap;
use futures::future::{join_all, select_all, BoxFuture};
use futures::{FutureExt, TryFutureExt};
use starlane::space::artifact::asynch::{ArtErr, ArtifactFetcher, Artifacts};
use starlane::space::command::direct::create::KindTemplate;
use starlane::space::err::{HyperSpatialError, SpaceErr, SpatialError};
use starlane::space::hyper::{InterchangeKind, Knock};
use starlane::space::kind::{BaseKind, Kind, StarSub};
use starlane::space::loc::{Layer, MachineName, StarHandle, StarKey, Surface, ToPoint, ToSurface};
use starlane::space::log::{PointLogger, RootLogger};
use starlane::space::particle::property::PropertiesConfig;
use starlane::space::particle::{Property, Status, Stub};
use starlane::space::point::Point;
use starlane::space::selector::{KindSelector, Selector};
use starlane::space::settings::Timeouts;
use starlane::space::substance::{Bin, Substance};
use starlane::space::util::{OptSelector, ValuePattern};
use starlane::space::wave::core::cmd::CmdMethod;
use starlane::space::wave::exchange::asynch::Exchanger;
use starlane::space::wave::{Agent, DirectedProto, PongCore, WaveVariantDef};
use thiserror::Error;
use tokio::sync::oneshot::error::RecvError;
use tokio::sync::{broadcast, mpsc, oneshot, watch};

#[derive(Clone)]
pub struct MachineApi {
    tx: mpsc::Sender<MachineCall>,
    pub artifacts: Artifacts,
    pub registry: Registry,
    pub data_dir: String,
}

impl MachineApi {
    pub fn new<P>(
        tx: mpsc::Sender<MachineCall>,
        registry: Registry,
        artifacts: Artifacts,
        platform: &P,
    ) -> Self
    where
        P: Platform,
    {
        let data_dir = platform.data_dir();
        Self {
            tx,
            registry,
            artifacts,
            data_dir,
        }
    }
    pub async fn properties_config(&self, kind: &Kind) -> Result<PropertiesConfig, MachineErr> {
        let (rtn, rx) = tokio::sync::oneshot::channel();
        self.tx
            .send(MachineCall::PropertiesConfig {
                kind: kind.clone(),
                rtn,
            })
            .await?;

        Ok(rx.await?)
    }
    pub async fn select_kind(&self, template: &KindTemplate) -> Result<Kind, MachineErr> {
        let (rtn, rx) = tokio::sync::oneshot::channel();
        self.tx
            .send(MachineCall::SelectKind {
                template: template.clone(),
                rtn,
            })
            .await?;

        Ok(rx.await??)
    }

    pub async fn select_service(
        &self,
        selector: ServiceSelector,
    ) -> Result<ServiceTemplate, ServiceErr> {
        let (rtn, rx) = tokio::sync::oneshot::channel();
        let selector = MachineCall::SelectService { selector, rtn };
        self.tx.send(selector).await.unwrap();
        Ok(rx.await??)
    }

    pub async fn endpoint_factory(
        &self,
        from: StarKey,
        to: StarKey,
    ) -> Result<Box<dyn HyperwayEndpointFactory>, MachineErr> {
        let (rtn, mut rtn_rx) = oneshot::channel();
        self.tx
            .send(MachineCall::EndpointFactory { from, to, rtn })
            .await;
        Ok(rtn_rx.await?)
    }

    pub async fn add_interchange(
        &self,
        kind: InterchangeKind,
        gate: Arc<dyn HyperGate>,
    ) -> Result<(), SpaceErr> {
        let (rtn, rtn_rx) = oneshot::channel();
        self.tx
            .send(MachineCall::AddGate { kind, gate, rtn })
            .await?;
        rtn_rx.await?
    }

    pub async fn knock(&self, knock: Knock) -> Result<HyperwayEndpoint, SpaceErr> {
        let (rtn, rtn_rx) = oneshot::channel();
        self.tx.send(MachineCall::Knock { knock, rtn }).await;
        rtn_rx.await?
    }

    pub fn terminate(&self) {
        self.tx.try_send(MachineCall::Terminate);
    }

    pub async fn wait_ready(&self) {
        let (tx, mut rx) = oneshot::channel();
        self.tx.send(MachineCall::WaitForReady(tx)).await;
        rx.await;
    }

    pub async fn await_termination(&self) -> Result<(), String> {
        let (tx, mut rx) = oneshot::channel();
        self.tx.send(MachineCall::AwaitTermination(tx)).await;
        let mut rx = match rx.await {
            Ok(rx) => rx,
            Err(err) => {
                return Err(err.to_string());
            }
        };
        rx.recv().await.unwrap()
    }

    #[cfg(test)]
    pub async fn get_machine_star(&self) -> Result<HyperStarApi, SpaceErr> {
        let (tx, mut rx) = oneshot::channel();
        self.tx.send(MachineCall::GetMachineStar(tx)).await;
        Ok(rx.await?)
    }

    #[cfg(test)]
    pub async fn get_star(&self, key: StarKey) -> Result<HyperStarApi, SpaceErr> {
        let (rtn, mut rtn_rx) = oneshot::channel();
        self.tx.send(MachineCall::GetStar { key, rtn }).await;
        rtn_rx.await?
    }
}

#[derive(Clone)]
pub struct MachineSkel<P>
where
    P: Platform,
{
    pub name: MachineName,
    pub platform: P,
    pub registry: Registry,
    pub artifacts: Artifacts,
    pub logger: RootLogger,
    pub timeouts: Timeouts,
    pub api: MachineApi,
    pub status_rx: watch::Receiver<MachineStatus>,
    pub status_tx: mpsc::Sender<MachineStatus>,
    pub machine_star: Surface,
    pub global: Surface,
}

pub struct Machine<P>
where
    P: Platform + 'static,
{
    pub skel: MachineSkel<P>,
    pub stars: Arc<HashMap<Point, HyperStarApi>>,
    pub machine_star: HyperStarApi,
    pub gate_selector: Arc<HyperGateSelector>,
    pub call_tx: mpsc::Sender<MachineCall>,
    pub call_rx: mpsc::Receiver<MachineCall>,
    pub termination_broadcast_tx: broadcast::Sender<Result<(), String>>,
    pub logger: PointLogger,
}

impl<P> Machine<P>
where
    P: Platform + 'static,
{
    pub async fn new_api(platform: P) -> Result<MachineApi, P::Err> {
        let (call_tx, call_rx) = mpsc::channel(1024);
        let artifacts = platform.artifact_hub();
        let registry = platform.global_registry().await?;
        let machine_api = MachineApi::new(call_tx.clone(), registry, artifacts, &platform);
        tokio::spawn(async move { Machine::init(platform, call_tx, call_rx).await });

        Ok(machine_api)
    }

    async fn init(
        platform: P,
        call_tx: mpsc::Sender<MachineCall>,
        call_rx: mpsc::Receiver<MachineCall>,
    ) -> Result<MachineApi, HyperErr2> {
        let template = platform.machine_template();
        let machine_name = platform.machine_name();
        let artifacts = platform.artifact_hub();
        let registry = platform.global_registry().await?;
        let machine_api = MachineApi::new(call_tx.clone(), registry, artifacts, &platform);
        let (mpsc_status_tx, mut mpsc_status_rx) = mpsc::channel(128);
        let (watch_status_tx, watch_status_rx) = watch::channel(MachineStatus::Init);
        tokio::spawn(async move {
            while let Some(status) = mpsc_status_rx.recv().await {
                watch_status_tx.send(status);
            }
        });


        let machine_star = StarKey::machine(machine_name.clone())
            .to_point()
            .to_surface()
            .with_layer(Layer::Gravity);
        let logger = platform.logger().point(machine_star.point.clone());
        let global = machine_star
            .point
            .push("global")
            .unwrap()
            .to_surface()
            .with_layer(Layer::Core);
        let registry = logger.result(platform.global_registry().await)?;

        let skel = MachineSkel {
            name: machine_name.clone(),
            machine_star,
            registry: platform.global_registry().await?,
            artifacts: platform.artifact_hub(),
            logger: platform.logger(),
            timeouts: Timeouts::default(),
            platform: platform.clone(),
            api: machine_api.clone(),
            status_tx: mpsc_status_tx,
            status_rx: watch_status_rx,
            global,
        };

        let mut stars = HashMap::new();
        let mut gates = Arc::new(DashMap::new());
        let star_templates = template.with_machine_star(machine_name);

        for star_template in star_templates {
            let star_point = star_template.key.clone().to_point();
            let star_port = star_point.clone().to_surface().with_layer(Layer::Core);

            let drivers_point = star_point.push("drivers".to_string()).unwrap();
            let logger = skel.logger.point(drivers_point.clone());

            let mut star_tx: HyperStarTx = HyperStarTx::new(star_point.clone());
            let star_skel =
                HyperStarSkel::new(star_template.clone(), skel.clone(), &mut star_tx).await;

            let mut drivers = platform.drivers_builder(&star_template.kind);

            let mut interchange =
                HyperwayInterchange::new(logger.push_point("interchange").unwrap());

            let star_hop = star_point.clone().to_surface().with_layer(Layer::Gravity);

            let mut hyperway = Hyperway::new(star_hop.clone(), Agent::HyperUser, logger.clone());
            hyperway.transform_inbound(Box::new(LayerTransform::new(Layer::Gravity)));

            let hyperway_endpoint = hyperway.hyperway_endpoint_far(None).await;
            interchange.add(hyperway).await;
            interchange.singular_to(star_hop.clone());

            let interchange = Arc::new(interchange);
            let auth = skel.platform.star_auth(&star_template.key)?;
            let greeter = SimpleGreeter::new(star_hop.clone(), star_port.clone());
            let gate: Arc<dyn HyperGate> = Arc::new(MountInterchangeGate::new(
                auth,
                greeter,
                interchange.clone(),
                logger.clone(),
            ));

            for con in star_template.connections.iter() {
                match con {
                    StarCon::Receiver(remote) => {
                        let star = remote
                            .key
                            .clone()
                            .to_point()
                            .to_surface()
                            .with_layer(Layer::Gravity);
                        let hyperway = Hyperway::new(star, Agent::HyperUser, logger.clone());
                        interchange.add(hyperway).await;
                    }
                    StarCon::Connector(remote) => {
                        let star = remote
                            .key
                            .clone()
                            .to_point()
                            .to_surface()
                            .with_layer(Layer::Gravity);
                        let hyperway = Hyperway::new(star, Agent::HyperUser, logger.clone());
                        interchange.add(hyperway).await;
                    }
                }
            }

            gates.insert(InterchangeKind::Star(star_template.key.clone()), gate);
            let star_api = HyperStar::new(
                star_skel.clone(),
                drivers,
                hyperway_endpoint,
                interchange.clone(),
                star_tx,
            )
            .await?;
            stars.insert(star_point.clone(), star_api);
        }

        let mut gate_selector = Arc::new(HyperGateSelector::new(gates));
        skel.platform.start_services(&gate_selector).await;
        let gate: Arc<dyn HyperGate> = gate_selector.clone();

        let (machine_point, machine_star) = stars
            .iter()
            .find(|(k, v)| v.kind == StarSub::Machine)
            .map(|(k, v)| (k.clone(), v.clone()))
            .expect("expected Machine Star");

        {
            let machine_api = skel.api.clone();
            ctrlc::set_handler(move || {
                machine_api.terminate();
            });
        }

        let logger = skel.logger.point(machine_point);

        let (term_tx, _) = broadcast::channel(1);
        let stars = Arc::new(stars);
        {
            let mut star_statuses_rx: Vec<watch::Receiver<Status>> =
                stars.values().map(|s| s.status_rx.clone()).collect();
            let status_tx = skel.status_tx.clone();
            let star_count = stars.len();
            tokio::spawn(async move {
                loop {
                    let mut readies = 0;
                    let mut inits = 0;
                    let mut panics = 0;
                    let mut fatals = 0;
                    for status_rx in star_statuses_rx.iter_mut() {
                        match status_rx.borrow().clone() {
                            Status::Unknown => {}
                            Status::Pending => {}
                            Status::Init => {
                                inits = inits + 1;
                            }
                            Status::Ready => {
                                readies = readies + 1;
                            }
                            Status::Paused => {}
                            Status::Resuming => {}
                            Status::Panic => {
                                panics = panics + 1;
                            }
                            Status::Fatal => {
                                fatals = fatals + 1;
                            }
                            Status::Done => {}
                        }
                    }

                    if readies == star_count {
                        status_tx.send(MachineStatus::Ready).await;
                    } else if fatals > 0 {
                        status_tx.send(MachineStatus::Fatal).await;
                    } else if panics > 0 {
                        status_tx.send(MachineStatus::Panic).await;
                    } else if inits > 0 {
                        status_tx.send(MachineStatus::Init).await;
                    }

                    let boxed_status_rx: Vec<BoxFuture<Result<(), watch::error::RecvError>>> =
                        star_statuses_rx
                            .iter_mut()
                            .map(|s| s.changed().boxed())
                            .collect();
                    select_all(boxed_status_rx).await;
                }
            });
        }

        let mut machine = Self {
            skel: skel.clone(),
            logger: logger.clone(),
            machine_star,
            stars,
            gate_selector,
            call_tx,
            call_rx,
            termination_broadcast_tx: term_tx,
        };

        /// SETUP ARTIFAC
        let factory = MachineApiExtFactory {
            machine_api: machine_api.clone(),
            logger: logger.clone(),
        };
        let exchanger = Exchanger::new(
            Point::from_str("artifact").unwrap().to_surface(),
            Timeouts::default(),
            logger.clone(),
        );
        let client =
            HyperClient::new_with_exchanger(Box::new(factory), Some(exchanger), logger.clone())
                .unwrap();

        let fetcher = Arc::new(ClientArtifactFetcher::new(client, skel.registry.clone()));
        //        skel.artifacts.set_fetcher(fetcher).await;

        machine.start().await;
        Ok(machine_api)
    }

    async fn init0(&self) {
        let logger = self.logger.span();
        let mut inits = vec![];
        for star in self.stars.values() {
            inits.push(star.init().boxed());
        }

        join_all(inits).await;
    }

    async fn start(mut self) -> Result<(), HyperErr2> {
        self.call_tx
            .send(MachineCall::Init)
            .await
            .unwrap_or_default();

        while let Some(call) = self.call_rx.recv().await {
            match call {
                MachineCall::Init => {
                    self.init0().await;
                }
                MachineCall::Terminate => {
                    self.termination_broadcast_tx.send(Ok(()));
                    return Ok(());
                }
                MachineCall::AwaitTermination(tx) => {
println!("~~~ self.termination_broadcast_tx.is_empty(): {}", self.termination_broadcast_tx.is_empty());
                    tx.send(self.termination_broadcast_tx.subscribe());
println!("~~~ AwaitTermination processeed");
                }
                MachineCall::WaitForReady(rtn) => {
                    let mut status_rx = self.skel.status_rx.clone();
                    tokio::spawn(async move {
                        loop {
                            if MachineStatus::Ready == status_rx.borrow().clone() {
                                rtn.send(());
                                break;
                            }
                            match status_rx.changed().await {
                                Ok(_) => {}
                                Err(err) => {
                                    rtn.send(());
                                    break;
                                }
                            }
                        }
                    });
                }
                MachineCall::SelectKind { template, rtn } => {
                    rtn.send(self.skel.platform.select_kind(&template));
                }
                MachineCall::PropertiesConfig { kind, rtn } => {
                    rtn.send(self.skel.platform.properties_config(&kind));
                }
                MachineCall::AddGate { kind, gate, rtn } => {
                    rtn.send(self.gate_selector.add(kind.clone(), gate));
                }
                MachineCall::Knock { knock, rtn } => {
                    let gate_selector = self.gate_selector.clone();
                    let logger = self.skel.logger.point(self.skel.machine_star.point.clone());
                    tokio::spawn(async move {
                        rtn.send(
                            logger
                                .result_ctx("MachineCall::Knock", gate_selector.knock(knock).await),
                        );
                    });
                }
                MachineCall::EndpointFactory { from, to, rtn } => {
                    let factory = Box::new(MachineHyperwayEndpointFactory::new(
                        from,
                        to,
                        self.call_tx.clone(),
                    ));
                    rtn.send(factory).unwrap_or_default();
                }
                #[cfg(test)]
                MachineCall::GetMachineStar(rtn) => {
                    rtn.send(self.machine_star.clone());
                }
                #[cfg(test)]
                MachineCall::GetStar { key, rtn } => {
                    rtn.send(
                        self.stars
                            .get(&key.to_point())
                            .ok_or(format!("could not find star: {}", key.to_string()).into())
                            .cloned(),
                    )
                    .unwrap_or_default();
                }
                #[cfg(test)]
                MachineCall::GetRegistry(rtn) => {
                    rtn.send(self.skel.registry.clone());
                }
                MachineCall::SelectService { selector, rtn } => {
                    match self
                        .skel
                        .platform
                        .machine_template()
                        .services
                        .select_one(&selector)
                    {
                        None => rtn.send(Err(ServiceErr::NoTemplate(selector))),
                        Some(template) => rtn.send(Ok(template.clone())),
                    };
                }
            }


        }
        self.termination_broadcast_tx
            .send(Err(err!("machine quit unexpectedly."))?);
println!("MachineCall loop has exited");

        Ok(())
    }
}

#[derive(strum_macros::Display)]
pub enum MachineCall {
    Init,
    Terminate,
    AwaitTermination(oneshot::Sender<broadcast::Receiver<Result<(), String>>>),
    WaitForReady(oneshot::Sender<()>),
    AddGate {
        kind: InterchangeKind,
        gate: Arc<dyn HyperGate>,
        rtn: oneshot::Sender<Result<(), SpaceErr>>,
    },
    Knock {
        knock: Knock,
        rtn: oneshot::Sender<Result<HyperwayEndpoint, SpaceErr>>,
    },
    EndpointFactory {
        from: StarKey,
        to: StarKey,
        rtn: oneshot::Sender<Box<dyn HyperwayEndpointFactory>>,
    },
    SelectService {
        selector: ServiceSelector,
        rtn: oneshot::Sender<Result<ServiceTemplate, ServiceErr>>,
    },
    SelectKind {
        template: KindTemplate,
        rtn: oneshot::Sender<Result<Kind, SpaceErr>>,
    },
    PropertiesConfig {
        kind: Kind,
        rtn: oneshot::Sender<PropertiesConfig>,
    },
    #[cfg(test)]
    GetMachineStar(oneshot::Sender<HyperStarApi>),
    #[cfg(test)]
    GetStar {
        key: StarKey,
        rtn: oneshot::Sender<Result<HyperStarApi, SpaceErr>>,
    },
    #[cfg(test)]
    GetRegistry(oneshot::Sender<Registry>),
}

#[derive(Clone, Eq, PartialEq, strum_macros::Display)]
pub enum MachineStatus {
    Pending,
    Init,
    Ready,
    Panic,
    Fatal,
}

pub struct MachineTemplate {
    pub stars: Vec<StarTemplate>,
    pub services: Templates<ServiceTemplate>,
}

impl MachineTemplate {
    pub fn star_set(&self) -> HashSet<StarKey> {
        let mut rtn = HashSet::new();
        for star in self.stars.iter() {
            rtn.insert(star.key.clone());
        }
        rtn
    }

    pub fn with_machine_star(&self, machine: MachineName) -> Vec<StarTemplate> {
        let mut stars = self.stars.clone();
        let mut machine = StarTemplate::new(StarKey::machine(machine), StarSub::Machine);
        for star in stars.iter_mut() {
            star.connect(machine.to_stub());
            machine.receive(star.to_stub());
        }

        stars.push(machine);

        stars
    }
}

impl Default for MachineTemplate {
    fn default() -> Self {
        let constellation = "central".to_string();

        let mut central = StarTemplate::new(StarKey::central(), StarSub::Central);
        let mut nexus = StarTemplate::new(
            StarKey::new(&constellation, &StarHandle::name("nexus")),
            StarSub::Nexus,
        );
        let mut supe = StarTemplate::new(
            StarKey::new(&constellation, &StarHandle::name("super")),
            StarSub::Super,
        );
        let mut maelstrom = StarTemplate::new(
            StarKey::new(&constellation, &StarHandle::name("maelstrom")),
            StarSub::Maelstrom,
        );
        let mut scribe = StarTemplate::new(
            StarKey::new(&constellation, &StarHandle::name("scribe")),
            StarSub::Scribe,
        );
        let mut jump = StarTemplate::new(
            StarKey::new(&constellation, &StarHandle::name("jump")),
            StarSub::Jump,
        );
        let mut fold = StarTemplate::new(
            StarKey::new(&constellation, &StarHandle::name("fold")),
            StarSub::Fold,
        );

        nexus.receive(central.to_stub());
        nexus.receive(supe.to_stub());
        nexus.receive(maelstrom.to_stub());
        nexus.receive(scribe.to_stub());
        nexus.receive(jump.to_stub());
        nexus.receive(fold.to_stub());

        central.connect(nexus.to_stub());
        supe.connect(nexus.to_stub());
        maelstrom.connect(nexus.to_stub());
        scribe.connect(nexus.to_stub());
        jump.connect(nexus.to_stub());
        fold.connect(nexus.to_stub());

        let mut stars = vec![];
        stars.push(central);
        stars.push(nexus);
        stars.push(supe);
        stars.push(maelstrom);
        stars.push(scribe);
        stars.push(jump);
        stars.push(fold);

        let config = service_conf();
        let filestore = ServiceTemplate {
            name: "repo-filestore".to_string(),
            kind: ServiceKind::FileStore,
            driver: OptSelector::Selector(KindSelector::from_base(BaseKind::Repo)),
            config,
        };
        let services = Templates::new(vec![filestore]);

        Self { stars, services }
    }
}

pub struct MachineHyperwayEndpointFactory {
    from: StarKey,
    to: StarKey,
    call_tx: mpsc::Sender<MachineCall>,
}

impl MachineHyperwayEndpointFactory {
    pub fn new(from: StarKey, to: StarKey, call_tx: mpsc::Sender<MachineCall>) -> Self {
        Self { from, to, call_tx }
    }
}

#[async_trait]
impl HyperwayEndpointFactory for MachineHyperwayEndpointFactory {
    async fn create(
        &self,
        status_tx: mpsc::Sender<HyperConnectionDetails>,
    ) -> Result<HyperwayEndpoint, SpaceErr> {
        let knock = Knock::new(
            InterchangeKind::Star(self.to.clone()),
            self.from
                .clone()
                .to_point()
                .to_surface()
                .with_layer(Layer::Gravity),
            Substance::Empty,
        );
        let (rtn, mut rtn_rx) = oneshot::channel();
        self.call_tx.send(MachineCall::Knock { knock, rtn }).await;
        tokio::time::timeout(Duration::from_secs(60), rtn_rx).await??
    }
}

pub struct MachineApiExtFactory {
    pub machine_api: MachineApi,
    pub logger: PointLogger,
}

#[async_trait]
impl HyperwayEndpointFactory for MachineApiExtFactory {
    async fn create(
        &self,
        status_tx: mpsc::Sender<HyperConnectionDetails>,
    ) -> Result<HyperwayEndpoint, SpaceErr> {
        let knock = Knock {
            kind: InterchangeKind::DefaultControl,
            auth: Box::new(Substance::Empty),
            remote: None,
        };
        self.logger
            .result_ctx("machine_api.knock()", self.machine_api.knock(knock).await)
    }
}

pub struct ClientArtifactFetcher {
    pub registry: Registry,
    pub client: HyperClient,
}

impl ClientArtifactFetcher {
    pub fn new(client: HyperClient, registry: Registry) -> Self {
        Self { client, registry }
    }
}

#[async_trait]
impl ArtifactFetcher for ClientArtifactFetcher {
    async fn stub(&self, point: &Point) -> Result<Stub, ArtErr> {
        /*
        let record = self
            .registry
            .record(point)
            .await;
        Ok(record.details.stub)

         */
        todo!()
    }

    async fn fetch(&self, point: &Point) -> Result<Arc<Bin>, ArtErr> {
        let transmitter = self
            .client
            .transmitter_builder()
            .await
            .map_err(anyhow::Error::from)?
            .build();

        let mut wave = DirectedProto::ping();
        wave.method(CmdMethod::Read);
        wave.to(point.clone().to_surface().with_layer(Layer::Core));
        let pong: WaveVariantDef<PongCore> = transmitter
            .direct(wave)
            .await
            .map_err(anyhow::Error::from)?;

        pong.ok_or().err();

        if let Substance::Bin(bin) = pong.variant.core.body {
            Ok(Arc::new(bin))
        } else {
            Err(ArtErr::expecting(
                "Body Substance",
                "Bin",
                pong.variant.core.body.kind(),
            ))
        }
    }

    fn selector(&self) -> ValuePattern<Selector> {
        todo!()
    }
}

#[derive(Clone, Debug, Error)]
pub enum MachineErr {
    #[error(transparent)]
    SpaceErr(#[from] SpaceErr),
    #[error("no configured driver for kind template: '{0}'")]
    KindNotSupported(KindTemplate),
    #[error("tokio send error.")]
    TokioSendErr,
    #[error("tokio receive error '{0}'")]
    TokioReceiveErr(RecvError),
    #[error("{0}")]
    Anyhow(Arc<anyhow::Error>),
}

impl SpatialError for MachineErr {}

impl HyperSpatialError for MachineErr {}

impl<T> From<tokio::sync::mpsc::error::SendError<T>> for MachineErr {
    fn from(value: tokio::sync::mpsc::error::SendError<T>) -> Self {
        MachineErr::TokioSendErr
    }
}

impl From<tokio::sync::oneshot::error::RecvError> for MachineErr {
    fn from(value: tokio::sync::oneshot::error::RecvError) -> Self {
        MachineErr::TokioReceiveErr(value)
    }
}
