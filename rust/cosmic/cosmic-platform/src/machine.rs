use crate::global::GlobalDriverFactory;
use crate::star::{Star, StarApi, StarCon, StarSkel, StarTemplate, StarTx};
use crate::{DriversBuilder, PlatErr, Platform, Registry, RegistryApi};
use cosmic_api::error::MsgErr;
use cosmic_api::id::id::{Layer, Point, Port, ToPoint, ToPort};
use cosmic_api::id::{ConstellationName, MachineName, StarHandle, StarKey, StarSub};
use cosmic_api::log::{PointLogger, RootLogger};
use cosmic_api::particle::particle::Status;
use cosmic_api::quota::Timeouts;
use cosmic_api::substance::substance::Substance;
use cosmic_api::sys::{InterchangeKind, Knock};
use cosmic_api::wave::{Agent, HyperWave, UltraWave};
use cosmic_api::ArtifactApi;
use cosmic_hyperlane::{
    HyperClient, HyperConnectionErr, HyperGate, HyperGateSelector, HyperRouter, Hyperway,
    HyperwayExt, HyperwayInterchange, HyperwayStub, InterchangeGate, LayerTransform,
    LocalHyperwayGateJumper, LocalHyperwayGateUnlocker, MountInterchangeGate, SimpleGreeter,
    TokenAuthenticatorWithRemoteWhitelist,
};
use dashmap::DashMap;
use futures::future::{join_all, select_all, BoxFuture};
use futures::FutureExt;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::marker::PhantomData;
use std::process::Output;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::broadcast::Receiver;
use tokio::sync::oneshot::error::RecvError;
use tokio::sync::watch::Ref;
use tokio::sync::{broadcast, mpsc, oneshot, watch};
use tracing::info;

#[derive(Clone)]
pub struct MachineApi<P>
where
    P: Platform,
{
    tx: mpsc::Sender<MachineCall<P>>,
}

impl<P> MachineApi<P>
where
    P: Platform,
{
    pub fn new(tx: mpsc::Sender<MachineCall<P>>) -> Self {
        Self { tx }
    }

    pub async fn add_interchange(
        &self,
        kind: InterchangeKind,
        gate: Arc<dyn HyperGate>,
    ) -> Result<(), MsgErr> {
        let (rtn, rtn_rx) = oneshot::channel();
        self.tx.send(MachineCall::AddGate { kind, gate, rtn }).await;
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

    pub async fn wait(&self) -> Result<(), P::Err> {
        let (tx, mut rx) = oneshot::channel();
        self.tx.send(MachineCall::Wait(tx)).await;
        let mut rx = match rx.await {
            Ok(rx) => rx,
            Err(err) => {
                return Err(P::Err::new(err.to_string()));
            }
        };
        match rx.recv().await {
            Ok(result) => result,
            Err(err) => {
                return Err(P::Err::new(err.to_string()));
            }
        }
    }

    #[cfg(test)]
    pub async fn get_machine_star(&self) -> Result<StarApi<P>, MsgErr> {
        let (tx, mut rx) = oneshot::channel();
        self.tx.send(MachineCall::GetMachineStar(tx)).await;
        Ok(rx.await?)
    }
}

#[derive(Clone)]
pub struct MachineSkel<P>
where
    P: Platform,
{
    pub name: MachineName,
    pub platform: P,
    pub registry: Registry<P>,
    pub artifacts: ArtifactApi,
    pub logger: RootLogger,
    pub timeouts: Timeouts,
    pub api: MachineApi<P>,
    pub status_rx: watch::Receiver<MachineStatus>,
    pub status_tx: mpsc::Sender<MachineStatus>,
    pub machine_star: Port,
    pub global: Port,
}

pub struct Machine<P>
where
    P: Platform + 'static,
{
    pub skel: MachineSkel<P>,
    pub stars: Arc<HashMap<Point, StarApi<P>>>,
    pub machine_star: StarApi<P>,
    pub gate_selector: HyperGateSelector,
    pub call_tx: mpsc::Sender<MachineCall<P>>,
    pub call_rx: mpsc::Receiver<MachineCall<P>>,
    pub termination_broadcast_tx: broadcast::Sender<Result<(), P::Err>>,
    pub logger: PointLogger,
}

impl<P> Machine<P>
where
    P: Platform + 'static,
{
    pub fn new(platform: P) -> MachineApi<P> {
        let (call_tx, call_rx) = mpsc::channel(1024);
        let machine_api = MachineApi::new(call_tx.clone());
        tokio::spawn(async move { Machine::init(platform, call_tx, call_rx).await });

        machine_api
    }

    async fn init(
        platform: P,
        call_tx: mpsc::Sender<MachineCall<P>>,
        call_rx: mpsc::Receiver<MachineCall<P>>,
    ) -> Result<MachineApi<P>, P::Err> {
        let template = platform.machine_template();
        let machine_name = platform.machine_name();
        let machine_api = MachineApi::new(call_tx.clone());
        let (mpsc_status_tx, mut mpsc_status_rx) = mpsc::channel(128);
        let (watch_status_tx, watch_status_rx) = watch::channel(MachineStatus::Init);
        tokio::spawn(async move {
            while let Some(status) = mpsc_status_rx.recv().await {
                watch_status_tx.send(status);
            }
        });

        let machine_star = StarKey::machine(machine_name.clone())
            .to_point()
            .to_port()
            .with_layer(Layer::Gravity);
        let global = machine_star
            .point
            .push("global")
            .unwrap()
            .to_port()
            .with_layer(Layer::Core);
        let skel = MachineSkel {
            name: machine_name.clone(),
            machine_star,
            registry: platform.global_registry().await?,
            artifacts: platform.artifact_hub(),
            logger: RootLogger::default(),
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
            let star_port = star_point.clone().to_port().with_layer(Layer::Core);

            let drivers_point = star_point.push("drivers".to_string()).unwrap();
            let logger = skel.logger.point(drivers_point.clone());

            let mut star_tx: StarTx<P> = StarTx::new(star_point.clone());
            let star_skel = StarSkel::new(star_template.clone(), skel.clone(), &mut star_tx).await;

            let mut drivers = match star_template.kind {
                StarSub::Machine => {
                    let mut drivers = DriversBuilder::new();
                    drivers.add(Arc::new(GlobalDriverFactory::new(star_skel.clone())));
                    drivers
                }
                _ => platform.drivers_builder(&star_template.kind),
            };

            //            let drivers = builder.build(drivers_point.to_port(), star_skel.clone())?;

            let mut interchange = HyperwayInterchange::new(logger.push("interchange").unwrap());

            let star_hop = star_point.clone().to_port().with_layer(Layer::Gravity);

            let mut hyperway = Hyperway::new(star_hop.clone(), Agent::HyperUser);
            hyperway.transform_inbound(Box::new(LayerTransform::new(Layer::Gravity)));

            let hyperway_ext = hyperway.mount().await;
            interchange.add(hyperway);
            interchange.singular_to(star_port.clone());

            let interchange = Arc::new(interchange);
            let auth = skel.platform.star_auth(&star_template.key)?;
            let greeter = SimpleGreeter::new(star_hop, star_port.clone());
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
                            .to_port()
                            .with_layer(Layer::Gravity);
                        let hyperway = Hyperway::new(star, Agent::HyperUser);
                        interchange.add(hyperway);
                    }
                    StarCon::Connector(remote) => {
                        let star = remote
                            .key
                            .clone()
                            .to_point()
                            .to_port()
                            .with_layer(Layer::Gravity);
                        let hyperway = Hyperway::new(star, Agent::HyperUser);
                        interchange.add(hyperway);
                    }
                }
            }

            gates.insert(InterchangeKind::Star(star_template.key.clone()), gate);

            let star_api = Star::new(star_skel.clone(), drivers, hyperway_ext, star_tx).await?;
            stars.insert(star_point.clone(), star_api);
        }

        let mut gate_selector = HyperGateSelector::new(gates);

        skel.platform.start_services(&mut gate_selector);

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
            skel,
            logger,
            machine_star,
            stars,
            gate_selector,
            call_tx,
            call_rx,
            termination_broadcast_tx: term_tx,
        };

        machine.start().await;
        Ok(machine_api)
    }

    async fn init0(&self) {
        let logger = self.logger.span();
        logger.info("Machine::init_drivers()");
        let mut inits = vec![];
        for star in self.stars.values() {
            inits.push(star.init().boxed());
        }

        join_all(inits).await;
    }

    async fn start(mut self) -> Result<(), P::Err> {
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
                MachineCall::Wait(tx) => {
                    tx.send(self.termination_broadcast_tx.subscribe());
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
                MachineCall::AddGate { kind, gate, rtn } => {
                    rtn.send(self.gate_selector.add(kind.clone(), gate));
                }
                MachineCall::Knock { knock, rtn } => {
                    let gate_selector = self.gate_selector.clone();
                    tokio::spawn(async move {
                        rtn.send(gate_selector.knock(knock).await)
                            .unwrap_or_default();
                    });
                }
                #[cfg(test)]
                MachineCall::GetMachineStar(tx) => {
                    tx.send(self.machine_star.clone());
                }
                #[cfg(test)]
                MachineCall::GetRegistry(tx) => {}
            }

            self.termination_broadcast_tx
                .send(Err(P::Err::new("machine quit unexpectedly.")));
        }

        Ok(())
    }
}

pub enum MachineCall<P>
where
    P: Platform,
{
    Init,
    Terminate,
    Wait(oneshot::Sender<broadcast::Receiver<Result<(), P::Err>>>),
    WaitForReady(oneshot::Sender<()>),
    AddGate {
        kind: InterchangeKind,
        gate: Arc<dyn HyperGate>,
        rtn: oneshot::Sender<Result<(), MsgErr>>,
    },
    Knock {
        knock: Knock,
        rtn: oneshot::Sender<Result<HyperwayExt, HyperConnectionErr>>,
    },
    #[cfg(test)]
    GetMachineStar(oneshot::Sender<StarApi<P>>),
    #[cfg(test)]
    GetRegistry(oneshot::Sender<Registry<P>>),
}

#[derive(Clone, Eq, PartialEq)]
pub enum MachineStatus {
    Pending,
    Init,
    Ready,
    Panic,
    Fatal,
}

pub struct MachineTemplate {
    pub stars: Vec<StarTemplate>,
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

        Self { stars }
    }
}
