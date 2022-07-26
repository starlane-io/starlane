use crate::star::{Star, StarApi, StarCon, StarSkel, StarTemplate, StarTx};
use crate::{PlatErr, Platform, Registry, RegistryApi};
use cosmic_api::error::MsgErr;
use cosmic_api::id::id::{Layer, Point, ToPoint, ToPort};
use cosmic_api::id::{ConstellationName, MachineName, StarHandle, StarKey, StarSub};
use cosmic_api::log::{PointLogger, RootLogger};
use cosmic_api::quota::Timeouts;
use cosmic_api::substance::substance::Substance;
use cosmic_api::sys::{InterchangeKind, Knock};
use cosmic_api::wave::{Agent, HyperWave, UltraWave};
use cosmic_api::ArtifactApi;
use cosmic_hyperlane::{HyperClient, HyperConnectionErr, HyperGate, HyperGateSelector, HyperRouter, Hyperway, HyperwayExt, HyperwayInterchange, HyperwayStub, InterchangeGate, LayerTransform, LocalHyperwayGateJumper, LocalHyperwayGateUnlocker, MountInterchangeGate, SimpleGreeter, TokenAuthenticatorWithRemoteWhitelist};
use dashmap::DashMap;
use futures::future::{BoxFuture, join_all};
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::marker::PhantomData;
use std::process::Output;
use std::str::FromStr;
use std::sync::Arc;
use futures::FutureExt;
use tokio::sync::broadcast::Receiver;
use tokio::sync::oneshot::error::RecvError;
use tokio::sync::{broadcast, mpsc, oneshot};
use tracing::info;
use cosmic_api::particle::particle::Status;

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

    pub async fn add_interchange( &self, kind: InterchangeKind, gate: Arc<dyn HyperGate> ) -> Result<(),MsgErr> {
        let (rtn,rtn_rx) = oneshot::channel();
        self.tx.send( MachineCall::AddGate {kind, gate, rtn }).await;
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
    pub registry: Arc<dyn RegistryApi<P>>,
    pub artifacts: ArtifactApi,
    pub logger: RootLogger,
    pub timeouts: Timeouts,
    pub api: MachineApi<P>,
}

pub struct Machine<P>
where
    P: Platform + 'static,
{
    pub skel: MachineSkel<P>,
    pub stars: Arc<HashMap<Point, StarApi<P>>>,
    pub status: MachineStatus,
    pub status_broadcast: broadcast::Sender<MachineStatus>,
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
        let (tx, rx) = mpsc::channel(1024);
        let machine_api = MachineApi::new(tx.clone());
        tokio::spawn(async move { Machine::init(platform, tx, rx).await });

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
        let skel = MachineSkel {
            name: machine_name.clone(),
            registry: platform.global_registry().await?,
            artifacts: platform.artifact_hub(),
            logger: RootLogger::default(),
            timeouts: Timeouts::default(),
            platform,
            api: machine_api.clone(),
        };

        let mut stars = HashMap::new();
        let mut gates = Arc::new(DashMap::new());
        let star_templates = template.with_machine_star(machine_name);

        for star_template in star_templates {
            let star_point = star_template.key.clone().to_point();
            let star_port = star_point.clone().to_port().with_layer(Layer::Core);
            let mut drivers = skel.platform.drivers_builder(&star_template.kind);
            let drivers_point = star_point.push("drivers".to_string()).unwrap();
            let logger = skel.logger.point(drivers_point.clone());
            drivers.logger.replace(logger.clone());

            let mut star_tx: StarTx<P> = StarTx::new(star_point.clone());
            let call_tx = star_tx.call_tx.clone();
            let call_rx = star_tx.star_rx().unwrap();
            let star_skel = StarSkel::new(star_template.clone(), skel.clone(), drivers.kinds(), star_tx );
//            let drivers = builder.build(drivers_point.to_port(), star_skel.clone())?;

            let mut interchange = HyperwayInterchange::new(
                logger.push("interchange").unwrap(),
            );

            let star_hop = star_point.clone().to_port().with_layer(Layer::Gravity);

            let mut hyperway = Hyperway::new(star_hop.clone(), Agent::HyperUser );
            hyperway.transform_inbound(Box::new(LayerTransform::new(Layer::Gravity)));

            let hyperway_ext = hyperway.mount().await;
            interchange.add(hyperway);
            interchange.singular_to(star_port.clone() );

            let interchange = Arc::new(interchange);
            let auth = skel.platform.star_auth(&star_template.key)?;
            let greeter = SimpleGreeter::new(star_hop, star_port.clone() );
            let gate: Arc<dyn HyperGate> = Arc::new(MountInterchangeGate::new(auth, greeter, interchange.clone(), logger.clone()));

            for con in star_template.connections.iter() {
                match con {
                    StarCon::Receiver(remote) => {
                        let star = remote.key.clone().to_point().to_port().with_layer(Layer::Gravity);
                        let hyperway = Hyperway::new(star, Agent::HyperUser);
                        interchange.add(hyperway);
                    }
                    StarCon::Connector(remote) => {
                        let star = remote.key.clone().to_point().to_port().with_layer(Layer::Gravity);
                        let hyperway = Hyperway::new(star, Agent::HyperUser);
                        interchange.add(hyperway);
                    }
                }
            }

            gates.insert(
                InterchangeKind::Star(star_template.key.clone()),
                gate,
            );


            let star_api = Star::new(star_skel.clone(), drivers, hyperway_ext, call_tx, call_rx ).await?;
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

        let (status_broadcast,_) = broadcast::channel(128);
        let mut machine = Self {
            skel,
            logger,
            machine_star,
            stars: Arc::new(stars),
            gate_selector,
            call_tx,
            call_rx,
            termination_broadcast_tx: term_tx,
            status: MachineStatus::Pending,
            status_broadcast
        };

        machine.start().await;
        Ok(machine_api)
    }

    async fn init0(&self)  {
        let logger = self.logger.span();
        logger.info("Machine::init0()");
        let mut inits0 = vec![];
        for star in self.stars.values() {
            inits0.push(star.init0().boxed());
        }

        /*
        struct Init0<'a,P> where P: Platform+'static {
            pub inits: Vec<BoxFuture<'a, Result<Status,<P as Platform>::Err>>>,
            pub logger: PointLogger,
            pub call_tx: mpsc::Sender<MachineCall<P>>
        }

        impl <'a,P> Init0<'a,P> where P:Platform+'static{
            pub fn new( inits: Vec<BoxFuture<'a, Result<Status,P::Err>>>, logger: PointLogger, call_tx: mpsc::Sender<MachineCall<P>> ) -> Self {
                Self {
                    inits,
                    logger,
                    call_tx
                }
            }

            pub async fn join_all(mut self){
                let results = join_all(self.inits).await;
                for result in results {
                    if result.is_err() {
                        let err = result.unwrap_err();
                        self.logger.error(format!("init error in star: {}", err.to_string()));
                        self.call_tx.send(MachineCall::SetStatus(MachineStatus::Panic)).await.unwrap_or_default();
                        return;
                    }
                }
                self.call_tx.send( MachineCall::SetStatus(MachineStatus::Ready)).await.unwrap_or_default();
            }
        }


        let mut inits = Init0::new(inits0, self.logger.clone(), self.call_tx.clone() );
         */

        let call_tx = self.call_tx.clone();
        let logger = self.logger.clone();
        tokio::spawn( async move {
//            let results = join_all(inits0).await;
/*            for result in results {
                if result.is_err() {
                    let err = result.unwrap_err();
                    logger.error(format!("init error in star: {}", err.to_string()));
                    call_tx.send(MachineCall::SetStatus(MachineStatus::Panic)).await.unwrap_or_default();
                    return;
                }
            }

 */
        });
    }

    async fn start(mut self) -> Result<(), P::Err> {
        self.call_tx.send(MachineCall::Init0).await.unwrap_or_default();

        while let Some(call) = self.call_rx.recv().await {
            match call {
                MachineCall::Init0 => {

                    let stars = self.stars.clone();
                    let call_tx = self.call_tx.clone();
                    let logger = self.logger.clone();

                    tokio::spawn( async move {
                        let mut inits0= vec![];
                        for (_,star) in stars.iter() {
                            inits0.push(star.init0().boxed());
                        }
                        let results = join_all(inits0).await;
                        for result in results {
                            if result.is_err() {
                                let err = result.unwrap_err();
                                logger.error(format!("init error in star: {}", err.to_string()));
                                call_tx.send(MachineCall::SetStatus(MachineStatus::Panic)).await.unwrap_or_default();
                                return;
                            }
                        }
                        call_tx.send(MachineCall::SetStatus(MachineStatus::Ready)).await.unwrap_or_default();
                    });
                }
                MachineCall::Terminate => {
                    self.termination_broadcast_tx.send(Ok(()));
                    return Ok(());
                }
                MachineCall::Wait(tx) => {
                    tx.send(self.termination_broadcast_tx.subscribe());
                }

                MachineCall::WaitForReady(tx) => {
                    // right now we don't know what Ready means other than the Call loop has started
                    // so we return Ready in every case
                    if self.status == MachineStatus::Ready {
                        tx.send(());
                    } else {
                        let mut rx = self.status_broadcast.subscribe();
                        tokio::spawn(async move {
                            while let Ok(status) = rx.recv().await {
                                if status == MachineStatus::Ready {
                                    tx.send(());
                                    break;
                                }
                            }
                        });
                    }
                }
                MachineCall::Phantom(_) => {
                    // do nothing, it is just here to carry the 'P'
                }
                MachineCall::AddGate { kind, gate, rtn } => {
                    rtn.send(self.gate_selector.add(kind.clone(),gate));
                }
                MachineCall::SetStatus(status) => {
                    self.status = status.clone();
                    self.status_broadcast.send(status);
                }
                MachineCall::Knock { knock, rtn } => {
                    let gate_selector = self.gate_selector.clone();
                    tokio::spawn( async move {
                        rtn.send(gate_selector.knock(knock).await).unwrap_or_default();
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
    Init0,
    Terminate,
    Wait(oneshot::Sender<broadcast::Receiver<Result<(), P::Err>>>),
    WaitForReady(oneshot::Sender<()>),
    AddGate { kind: InterchangeKind, gate: Arc<dyn HyperGate>, rtn: oneshot::Sender<Result<(),MsgErr>> },
    Knock{ knock: Knock, rtn: oneshot::Sender<Result<HyperwayExt,HyperConnectionErr>> },
    Phantom(PhantomData<P>),
    SetStatus(MachineStatus),
    #[cfg(test)]
    GetMachineStar(oneshot::Sender<StarApi<P>>),
    #[cfg(test)]
    GetRegistry(oneshot::Sender<Registry<P>>),
}

#[derive(Clone,Eq,PartialEq)]
pub enum MachineStatus {
    Pending,
    Init0,
    Ready,
    Panic,
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
        let mut machine = StarTemplate::new(
            StarKey::new(&"machine".to_string(), &StarHandle::name(machine.as_str())),
            StarSub::Machine,
        );
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
