use crate::star::{Star, StarApi, StarCon, StarSkel, StarTemplate};
use crate::{PlatErr, Platform, Registry, RegistryApi};
use cosmic_api::error::MsgErr;
use cosmic_api::id::id::{Point, ToPoint, ToPort};
use cosmic_api::id::{ConstellationName, MachineName, StarHandle, StarKey, StarSub};
use cosmic_api::log::{PointLogger, RootLogger};
use cosmic_api::quota::Timeouts;
use cosmic_api::substance::substance::Substance;
use cosmic_api::sys::{InterchangeKind, Knock};
use cosmic_api::wave::{Agent, HyperWave, UltraWave};
use cosmic_api::ArtifactApi;
use cosmic_hyperlane::{HyperClient, HyperGate, HyperGateSelector, HyperRouter, Hyperway, HyperwayInterchange, HyperwayStub, InterchangeGate, LocalHyperwayGateJumper, LocalHyperwayGateUnlocker, MountInterchangeGate, TokenAuthenticatorWithRemoteWhitelist};
use dashmap::DashMap;
use futures::future::join_all;
use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;
use std::sync::Arc;
use tokio::sync::broadcast::Receiver;
use tokio::sync::oneshot::error::RecvError;
use tokio::sync::{broadcast, mpsc, oneshot};
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
    pub machine_star: StarApi<P>,
    pub gate_selector: HyperGateSelector,
    pub rx: mpsc::Receiver<MachineCall<P>>,
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
        tx: mpsc::Sender<MachineCall<P>>,
        rx: mpsc::Receiver<MachineCall<P>>,
    ) -> Result<MachineApi<P>, P::Err> {
        let template = platform.machine_template();
        let machine_name = platform.machine_name();

        let machine_api = MachineApi::new(tx);
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
            let mut builder = skel.platform.drivers_builder(&star_template.kind);
            let drivers_point = star_point.push("drivers".to_string()).unwrap();
            let logger = skel.logger.point(drivers_point.clone());
            builder.logger.replace(logger.clone());
            let star_skel = StarSkel::new(star_template.clone(), skel.clone(), builder.kinds());
            let drivers = builder.build(drivers_point.to_port(), star_skel.clone())?;

            let interchange = Arc::new(HyperwayInterchange::new(
                logger.push("interchange").unwrap(),
            ));


                /*
            {
                let router = interchange.router();
                tokio::spawn(async move {
                    while let Some(wave) = fabric_rx.recv().await {
                        println!("ROUTING TO FABRIC!");
                        router.route(wave).await;
                    }
                });
            }
                 */

            let auth = skel.platform.star_auth(&star_template.key)?;
            let gate: Arc<dyn HyperGate> = Arc::new(MountInterchangeGate::new(auth, interchange.clone(), logger.clone()));
            let hyperway = Hyperway::new(star_point.clone(), Agent::HyperUser );
            let hyperway_ext = hyperway.mount().await;
            interchange.add(hyperway);

            for con in star_template.connections.iter() {
                match con {
                    StarCon::Receiver(remote) => {
                        let star = remote.key.clone().to_point();
                        let hyperway = Hyperway::new(star, Agent::HyperUser);
                        interchange.add(hyperway);
                    }
                    StarCon::Connector(remote) => {
                        let star = remote.key.clone().to_point();
                        let hyperway = Hyperway::new(star, Agent::HyperUser);
                        interchange.add(hyperway);
                    }
                }
            }

            gates.insert(
                InterchangeKind::Star(star_template.key.clone()),
                gate,
            );

            let star_api = Star::new(star_skel.clone(), drivers, hyperway_ext)?;
            stars.insert(star_point.clone(), star_api);

        }

        let mut entry_router = HyperGateSelector::new(gates);

        /*
        // now lets make the clients
        for (from, to) in clients {
            let entry_req = Knock {
                kind: InterchangeKind::Star(to.clone()),
                auth: Box::new(Substance::Token(skel.platform.token())),
                remote: Some(from.clone()),
            };

            let logger = skel.logger.point(from.clone());
            let factory = LocalHyperwayExtFactory::new(entry_req, entry_router.clone());
            let hyperway =
                HyperClient::new(Agent::HyperUser, to.to_point(), Box::new(factory), logger)?;
            interchanges
                .get(&StarKey::try_from(from).unwrap())
                .unwrap()
                .add(hyperway);
        }

         */

        skel.platform.start_services(&mut entry_router);

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

        let mut machine = Self {
            skel,
            logger,
            machine_star,
            stars: Arc::new(stars),
            gate_selector: entry_router,
            rx,
            termination_broadcast_tx: term_tx,
        };

        machine.start().await;
        Ok(machine_api)
    }

    async fn pre_init(&self) -> Result<(), MsgErr> {
        let logger = self.logger.span();
        logger.info("Machine::pre_init()");
        let mut pre_inits = vec![];
        for star in self.stars.values() {
            pre_inits.push(star.pre_init());
        }
        let results: Vec<Result<(), MsgErr>> = join_all(pre_inits).await;
        for result in results {
            if result.is_err() {
                logger.error("init error in star");
                result?;
            }
        }
        // for now we don't really check or do anything with Drivers
        Ok(())
    }

    async fn start(mut self) -> Result<(), P::Err> {
        //        self.pre_init().await?;

        while let Some(call) = self.rx.recv().await {
            match call {
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
                    tx.send(());
                }
                MachineCall::Phantom(_) => {
                    // do nothing, it is just here to carry the 'P'
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
    Terminate,
    Wait(oneshot::Sender<broadcast::Receiver<Result<(), P::Err>>>),
    WaitForReady(oneshot::Sender<()>),
    Phantom(PhantomData<P>),
    #[cfg(test)]
    GetMachineStar(oneshot::Sender<StarApi<P>>),
    #[cfg(test)]
    GetRegistry(oneshot::Sender<Registry<P>>),
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
