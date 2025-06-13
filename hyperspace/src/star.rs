use crate::driver::star::{StarDiscovery, StarPair, StarWrangles, Wrangler};
use crate::driver::{DriverErr, DriverStatus, DriversApi, DriversBuilder, DriversCall};
use crate::global::{GlobalCommandExecutionHandler, GlobalExecutionChamber};
use crate::hyperlane::{
    Bridge, HyperwayEndpoint, HyperwayEndpointFactory, HyperwayInterchange, HyperwayStub,
};
use crate::layer::field::Field;
use crate::layer::shell::{Shell, ShellState};
use crate::machine::{MachineApi, MachineErr, MachineSkel};
use crate::base::Platform;
use crate::registry::err::RegErr;
use crate::registry::{Registration, Registry};
use crate::service::ServiceTemplate;
use crate::template::Templates;
use anyhow::{Context, Error};
use async_recursion::async_recursion;
use async_trait::async_trait;
use dashmap::DashMap;
use itertools::Itertools;
use starlane_macros::{log_span, push_loc, push_mark};
use starlane_space::command::common::StateSrc;
use starlane_space::command::direct::create::{Create, Strategy};
use starlane_space::err::{CoreReflector, ParseErrs, SpaceErr, SpatialError};
use starlane_space::hyper::{Assign, AssignmentKind, HyperSubstance, Provision, Search};
use starlane_space::hyper::{MountKind, ParticleLocation};
use starlane_space::kind::{Kind, StarStub, StarSub};
use starlane_space::loc::{
    Layer, StarKey, Surface, SurfaceSelector, ToPoint, ToSurface, GLOBAL_EXEC,
};
use starlane_space::log::{Logger, Trackable, Tracker};
use starlane_space::particle::traversal::{
    Traversal, TraversalDirection, TraversalInjection, TraversalLayer,
};
use starlane_space::particle::{Details, Status};
use starlane_space::point::Point;
use starlane_space::substance::{Substance, SubstanceErr, SubstanceKind};
use starlane_space::util::ValueMatcher;
use starlane_space::wave::core::cmd::CmdMethod;
use starlane_space::wave::core::hyper::HypMethod;
use starlane_space::wave::core::ReflectedCore;
use starlane_space::wave::exchange::asynch::{
    DirectedHandler, DirectedHandlerShell, Exchanger, ProtoTransmitter, ProtoTransmitterBuilder,
    Router, TraversalRouter, TxRouter,
};
use starlane_space::wave::exchange::SetStrategy;
use starlane_space::wave::Wave;
use starlane_space::wave::{
    Agent, DirectedProto, Handling, HandlingKind, PongCore, Priority, Recipients, Reflectable,
    ReflectedWave, Retries, Ripple, Scope, SignalCore, SingularRipple, ToReflected, WaitTime,
    WaveId, WaveKind, WaveVariantDef,
};
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::marker::PhantomData;
use std::ops::Add;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::{broadcast, mpsc, oneshot, watch};

#[derive(Clone)]
pub struct ParticleStates {
    topic: Arc<DashMap<Surface, Arc<dyn TopicHandler>>>,
    shell: Arc<DashMap<Point, ShellState>>,
}

impl ParticleStates {
    pub fn create_shell(&self, point: Point) {
        self.shell.insert(point.clone(), ShellState::new(point));
    }

    pub fn new() -> Self {
        Self {
            topic: Arc::new(DashMap::new()),
            shell: Arc::new(DashMap::new()),
        }
    }

    pub fn topic_handler(&self, port: Surface, handler: Arc<dyn TopicHandler>) {
        self.topic.insert(port, handler);
    }

    pub fn find_topic(
        &self,
        port: &Surface,
        source: &Surface,
    ) -> Option<Result<Arc<dyn TopicHandler>, SpaceErr>> {
        match self.topic.get(port) {
            None => None,
            Some(topic) => {
                let topic = topic.value().clone();
                if topic.source_selector().is_match(source).is_ok() {
                    Some(Ok(topic))
                } else {
                    Some(Err(SpaceErr::not_found("topic")))
                }
            }
        }
    }

    pub fn find_shell(&self, point: &Point) -> Result<ShellState, SpaceErr> {
        Ok(self
            .shell
            .get(point)
            .ok_or(format!(
                "expected shell state for point: {}",
                point.to_string()
            ))?
            .value()
            .clone())
    }
}

#[derive(Clone)]
pub struct HyperStarSkel {
    pub api: HyperStarApi,
    pub machine_api: MachineApi,
    pub key: StarKey,
    pub point: Point,
    pub kind: StarSub,
    pub logger: Logger,
    pub registry: Registry,
    pub golden_path: Arc<DashMap<StarKey, StarKey>>,
    pub traverse_to_next_tx: mpsc::Sender<Traversal<Wave>>,
    pub inject_tx: mpsc::Sender<TraversalInjection>,
    //    pub machine: MachineSkel,
    pub exchanger: Exchanger,
    pub state: ParticleStates,
    pub adjacents: HashMap<Point, StarStub>,
    pub wrangles: StarWrangles,
    pub gravity_tx: mpsc::Sender<Wave>,
    pub gravity_router: TxRouter,
    pub gravity_transmitter: ProtoTransmitter,
    pub drivers: DriversApi,
    pub drivers_traversal_tx: mpsc::Sender<Traversal<Wave>>,
    pub status_tx: mpsc::Sender<Status>,
    pub status_rx: watch::Receiver<Status>,
    pub template: StarTemplate,
    pub star_transmitter: ProtoTransmitter,

    #[cfg(test)]
    pub diagnostic_interceptors: DiagnosticInterceptors,
}

impl HyperStarSkel {
    pub async fn new<P>(
        template: StarTemplate,
        machine: MachineSkel<P>,
        star_tx: &mut HyperStarTx,
    ) -> Self
    where
        P: Platform,
    {
        let point = template.key.clone().to_point();
        let logger = push_loc!((machine.logger, &point));
        let exchanger = Exchanger::new(
            point.clone().to_surface(),
            machine.timeouts.clone(),
            logger.clone(),
        );
        let state = ParticleStates::new();
        state.create_shell(point.clone());

        let registration = Registration {
            point: point.clone(),
            kind: Kind::Star(template.kind.clone()),
            registry: Default::default(),
            properties: Default::default(),
            owner: point.clone(),
            strategy: Strategy::Ensure,
            status: Status::Unknown,
        };

        machine.registry.register(&registration).await.unwrap();
        machine.registry.assign_star(&point, &point).await.unwrap();

        let api = HyperStarApi::new(
            template.kind.clone(),
            star_tx.call_tx.clone(),
            star_tx.status_rx.clone(),
        );

        let mut adjacents = HashMap::new();
        let mut golden_path = Arc::new(DashMap::new());
        // prime the searcher by mapping the immediate lanes
        for hyperway in template.connections.clone() {
            adjacents.insert(hyperway.key().clone().to_point(), hyperway.stub().clone());
            golden_path.insert(hyperway.key().clone(), hyperway.key().clone());
        }

        let gravity_router = TxRouter::new(star_tx.gravity_tx.clone());
        let mut gravity_transmitter =
            ProtoTransmitterBuilder::new(Arc::new(gravity_router.clone()), exchanger.clone());
        gravity_transmitter.from = SetStrategy::Override(point.clone().to_surface());
        gravity_transmitter.handling = SetStrategy::Fill(Handling {
            kind: HandlingKind::Immediate,
            priority: Priority::High,
            retries: Retries::None,
            wait: WaitTime::Low,
        });
        gravity_transmitter.agent = SetStrategy::Fill(Agent::HyperUser);
        gravity_transmitter.scope = SetStrategy::Fill(Scope::Full);

        let gravity_transmitter = gravity_transmitter.build();

        let drivers = DriversApi::new(
            star_tx.drivers_call_tx.clone(),
            star_tx.drivers_status_rx.clone(),
        );

        let star_router = LayerInjectionRouter::injector(
            star_tx.inject_tx.clone(),
            point.to_surface().with_layer(Layer::Core),
        );
        let mut star_transmitter =
            ProtoTransmitterBuilder::new(Arc::new(star_router), exchanger.clone());
        star_transmitter.from = SetStrategy::Override(point.to_surface().with_layer(Layer::Core));
        star_transmitter.agent = SetStrategy::Override(Agent::HyperUser);
        let star_transmitter = star_transmitter.build();

        Self {
            api,
            machine_api: machine.api.clone(),
            key: template.key.clone(),
            point,
            kind: template.kind.clone(),
            logger,
            golden_path,
            gravity_tx: star_tx.gravity_tx.clone(),
            gravity_router,
            gravity_transmitter,
            traverse_to_next_tx: star_tx.traverse_to_next_tx.clone(),
            inject_tx: star_tx.inject_tx.clone(),
            exchanger,
            state,
            registry: machine.registry.clone(),
            adjacents,
            wrangles: StarWrangles::new(),
            drivers,
            drivers_traversal_tx: star_tx.drivers_traversal_tx.clone(),
            status_tx: star_tx.status_tx.clone(),
            status_rx: star_tx.status_rx.clone(),
            star_transmitter,
            #[cfg(test)]
            diagnostic_interceptors: DiagnosticInterceptors::new(),
            template,
        }
    }

    pub fn data_dir(&self) -> String {
        format!("{}/{}/", self.machine_api.data_dir, self.point.to_string())
    }

    /*
    pub async fn create_star_particle(&self, point: Point, kind: Kind ) -> Result<(),StarErr> {

        if !self.point.is_parent_of(&point) {
            return Err(StarErr::new(format!("create_star_particle must be a child of star. expected: {}+:**, encountered: {}", self.point.to_string(), point.to_string())));
        }

        let registration = Registration {
            point: point.clone(),
            kind,
            registry: Default::default(),
            properties: Default::default(),
            owner: self.point.clone(),
            strategy: Strategy::Ensure,
            status: Status::Ready
        };

        self.registry.register(&registration).await?;
        self.api.create_states(point.clone()).await?;
        self.registry.assign(&point).send(self.point.clone());
        Ok(())
    }

     */

    pub fn err(&self, err: &StarErr) {
        self.logger.error(err);
    }

    #[track_caller]
    pub async fn create_in_star(&self, create: Create) -> Result<Details, StarErr> {
        if self.point != create.template.point.parent
            && !self.point.is_parent_of(&create.template.point.parent)
        {
            Err(StarErr::point_not_in_star(
                &self.point,
                &create.template.point.parent,
            ))?;
        }

        let logger = push_mark!(self.logger);
        let global = GlobalExecutionChamber::new(self.clone());
        let details = global.create(&create, &Agent::HyperUser).await?;

        let assign_body = Assign::new(AssignmentKind::Create, details.clone(), StateSrc::None);
        let mut assign = DirectedProto::sys(
            self.point.clone().to_surface().with_layer(Layer::Core),
            HypMethod::Assign,
        );

        assign.body(assign_body.clone().into());
        let router = Arc::new(LayerInjectionRouter::new(
            self.clone(),
            self.point.clone().to_surface().with_layer(Layer::Shell),
        ));
        let mut transmitter = ProtoTransmitterBuilder::new(router, self.exchanger.clone());
        transmitter.from = SetStrategy::Override(self.point.to_surface().with_layer(Layer::Core));
        transmitter.agent = SetStrategy::Override(Agent::HyperUser);
        transmitter.handling = SetStrategy::Override(Handling {
            kind: HandlingKind::Durable,
            priority: Default::default(),
            retries: Default::default(),
            wait: WaitTime::High,
        });
        let transmitter = transmitter.build();
        let assign_result: WaveVariantDef<PongCore> = logger.result_ctx(
            "StarSkel::create(assign_result)",
            transmitter.direct(assign).await,
        )?;
        self.registry
            .assign_star(&details.stub.point, &self.point)
            .await?;
        let logger = push_mark!(logger);
        logger.result(assign_result.ok_or())?;
        Ok(details)
    }

    /*
    pub fn err<M: ToString>(&self, message: M) -> Result<(), StarErr> {
        self.logger.warn(message.to_string());
        return Err(err!("{}",message.to_string()))?;
    }

     */

    pub fn point(&self) -> &Point {
        &self.point
    }

    pub fn stub(&self) -> StarStub {
        StarStub::new(self.key.clone(), self.kind.clone())
    }
}

pub enum HyperStarCall {
    Init,
    CreateStates {
        point: Point,
        rtn: oneshot::Sender<()>,
    },
    Stub(oneshot::Sender<StarStub>),
    FromHyperway {
        wave: Wave,
        rtn: Option<oneshot::Sender<Result<(), SpaceErr>>>,
    },
    TraverseToNextLayer(Traversal<Wave>),
    LayerTraversalInjection(TraversalInjection),
    ToDriver(Traversal<Wave>),
    ToGravity(Wave),
    ToHyperway(WaveVariantDef<SignalCore>),
    Shard(Wave),
    StartWrangling,
    Wrangle(oneshot::Sender<Result<StarWrangles, SpaceErr>>),
    Bounce {
        key: StarKey,
        rtn: oneshot::Sender<Result<(), SpaceErr>>,
    },
    #[cfg(test)]
    GetSkel(oneshot::Sender<HyperStarSkel>),
}

pub struct HyperStarTx {
    pub gravity_tx: mpsc::Sender<Wave>,
    pub traverse_to_next_tx: mpsc::Sender<Traversal<Wave>>,
    pub inject_tx: mpsc::Sender<TraversalInjection>,
    pub drivers_traversal_tx: mpsc::Sender<Traversal<Wave>>,
    pub call_tx: mpsc::Sender<HyperStarCall>,
    pub call_rx: Option<mpsc::Receiver<HyperStarCall>>,
    pub drivers_call_tx: mpsc::Sender<DriversCall>,
    pub drivers_call_rx: Option<mpsc::Receiver<DriversCall>>,
    pub drivers_status_tx: Option<watch::Sender<DriverStatus>>,
    pub drivers_status_rx: watch::Receiver<DriverStatus>,
    pub status_tx: mpsc::Sender<Status>,
    pub status_rx: watch::Receiver<Status>,
}

impl HyperStarTx {
    pub fn new(point: Point) -> Self {
        let (gravity_tx, mut gravity_rx) = mpsc::channel(1024);
        let (inject_tx, mut inject_rx): (
            mpsc::Sender<TraversalInjection>,
            mpsc::Receiver<TraversalInjection>,
        ) = mpsc::channel(1024);
        let (traverse_to_next_tx, mut traverse_to_next_rx): (
            mpsc::Sender<Traversal<Wave>>,
            mpsc::Receiver<Traversal<Wave>>,
        ) = mpsc::channel(1024);
        let (drivers_traversal_tx, mut drivers_rx): (
            mpsc::Sender<Traversal<Wave>>,
            mpsc::Receiver<Traversal<Wave>>,
        ) = mpsc::channel(1024);
        let (drivers_call_tx, mut drivers_call_rx) = mpsc::channel(1024);
        let (drivers_status_tx, drivers_status_rx) = watch::channel(DriverStatus::Pending);
        let (mpsc_status_tx, mut mpsc_status_rx) = mpsc::channel(128);
        let (watch_status_tx, watch_status_rx) = watch::channel(Status::Pending);

        tokio::spawn(async move {
            while let Some(status) = mpsc_status_rx.recv().await {
                watch_status_tx.send(status);
            }
        });

        let (call_tx, call_rx) = mpsc::channel(1024);

        {
            let call_tx = call_tx.clone();
            tokio::spawn(async move {
                while let Some(wave) = gravity_rx.recv().await {
                    call_tx.send(HyperStarCall::ToGravity(wave)).await;
                }
            });
        }

        {
            let call_tx = call_tx.clone();
            tokio::spawn(async move {
                while let Some(traversal) = traverse_to_next_rx.recv().await {
                    match call_tx
                        .send(HyperStarCall::TraverseToNextLayer(traversal.clone()))
                        .await
                    {
                        Ok(_) => {}
                        Err(err) => {
                            println!("CALL TX ERR: {}", err.to_string());
                        }
                    }
                }
            });
        }

        {
            let call_tx = call_tx.clone();
            tokio::spawn(async move {
                while let Some(inject) = inject_rx.recv().await {
                    call_tx
                        .send(HyperStarCall::LayerTraversalInjection(inject))
                        .await;
                }
            });
        }

        {
            let call_tx = call_tx.clone();
            tokio::spawn(async move {
                while let Some(inject) = drivers_rx.recv().await {
                    match call_tx.send(HyperStarCall::ToDriver(inject)).await {
                        Ok(_) => {}
                        Err(_) => {
                            panic!("driveres not working");
                        }
                    }
                }
                panic!("======== DRIVERS RX STOPPED");
            });
        }

        Self {
            gravity_tx,
            traverse_to_next_tx,
            inject_tx,
            drivers_traversal_tx,
            call_tx,
            call_rx: Some(call_rx),
            drivers_call_tx,
            drivers_call_rx: Option::Some(drivers_call_rx),
            drivers_status_tx: Some(drivers_status_tx),
            drivers_status_rx,
            status_tx: mpsc_status_tx,
            status_rx: watch_status_rx,
        }
    }

    pub fn star_rx(&mut self) -> Option<mpsc::Receiver<HyperStarCall>> {
        self.call_rx.take()
    }
}

#[derive(Clone)]
pub struct HyperStarApi {
    pub kind: StarSub,
    tx: mpsc::Sender<HyperStarCall>,
    pub status_rx: watch::Receiver<Status>,
}

impl HyperStarApi {
    pub fn new(
        kind: StarSub,
        tx: mpsc::Sender<HyperStarCall>,
        status_rx: watch::Receiver<Status>,
    ) -> Self {
        Self {
            kind,
            tx,
            status_rx,
        }
    }

    pub fn status(&self) -> Status {
        self.status_rx.borrow().clone()
    }

    pub async fn wait_for_status(&mut self, status: Status) {
        loop {
            if self.status_rx.borrow().clone() == status {
                break;
            }
            self.status_rx.changed().await.unwrap();
        }
    }

    pub async fn init(&self) {
        self.tx.send(HyperStarCall::Init).await;
    }

    pub async fn wrangle(&self) -> Result<StarWrangles, SpaceErr> {
        let (rtn, mut rtn_rx) = oneshot::channel();
        self.tx.send(HyperStarCall::Wrangle(rtn)).await?;
        tokio::time::timeout(Duration::from_secs(5), rtn_rx).await??
    }

    pub async fn start_wrangling(&self) {
        self.tx.send(HyperStarCall::StartWrangling).await;
    }

    pub async fn bounce(&self, key: StarKey) -> Result<(), SpaceErr> {
        let (rtn, mut rtn_rx) = oneshot::channel();
        self.tx.send(HyperStarCall::Bounce { key, rtn }).await?;
        tokio::time::timeout(Duration::from_secs(5), rtn_rx).await??
    }

    pub async fn from_hyperway(&self, wave: Wave, results: bool) -> Result<(), SpaceErr> {
        match results {
            true => {
                let (tx, mut rx) = oneshot::channel();
                self.tx
                    .send(HyperStarCall::FromHyperway {
                        wave,
                        rtn: Some(tx),
                    })
                    .await;
                rx.await?
            }
            false => {
                self.tx
                    .send(HyperStarCall::FromHyperway { wave, rtn: None })
                    .await;
                Ok(())
            }
        }
    }

    pub async fn traverse_to_next_layer(&self, traversal: Traversal<Wave>) {
        self.tx
            .send(HyperStarCall::TraverseToNextLayer(traversal))
            .await;
    }

    pub async fn inject_traversal(&self, inject: TraversalInjection) {
        self.tx
            .send(HyperStarCall::LayerTraversalInjection(inject))
            .await;
    }

    pub async fn stub(&self) -> Result<StarStub, SpaceErr> {
        let (tx, rx) = oneshot::channel();
        self.tx.send(HyperStarCall::Stub(tx)).await;
        Ok(rx.await?)
    }

    pub async fn create_states(&self, point: Point) -> Result<(), SpaceErr> {
        let (rtn, rtn_rx) = oneshot::channel();
        self.tx
            .send(HyperStarCall::CreateStates { point, rtn })
            .await;
        rtn_rx.await?;
        Ok(())
    }

    #[cfg(test)]
    pub async fn get_skel(&self) -> Result<HyperStarSkel, SpaceErr> {
        let (tx, rx) = oneshot::channel();
        self.tx.send(HyperStarCall::GetSkel(tx)).await;
        Ok(rx.await?)
    }

    #[cfg(test)]
    pub async fn to_gravity(&self, wave: Wave) {
        self.tx.send(HyperStarCall::ToGravity(wave)).await;
    }

    pub async fn to_hyperway(&self, wave: WaveVariantDef<SignalCore>) {
        self.tx.send(HyperStarCall::ToHyperway(wave)).await;
    }
}

pub struct HyperStar {
    skel: HyperStarSkel,
    star_tx: mpsc::Sender<HyperStarCall>,
    star_rx: mpsc::Receiver<HyperStarCall>,
    drivers: DriversApi,
    injector: Surface,
    forwarders: Vec<Point>,
    hyperway_transmitter: ProtoTransmitter,
    gravity: Surface,
    hyper_router: Arc<dyn Router>,
    layer_traversal_engine: LayerTraversalEngine,
    global_handler: DirectedHandlerShell<GlobalCommandExecutionHandler>,
}

impl HyperStar {
    pub async fn new(
        skel: HyperStarSkel,
        mut drivers: DriversBuilder,
        mut hyperway_endpoint: HyperwayEndpoint,
        interchange: Arc<HyperwayInterchange>,
        mut star_tx: HyperStarTx,
    ) -> Result<HyperStarApi, StarErr> {
        let drivers = drivers.build(
            skel.clone(),
            star_tx.drivers_call_tx.clone(),
            star_tx.drivers_call_rx.take().unwrap(),
            star_tx.drivers_status_tx.take().unwrap(),
            star_tx.drivers_status_rx.clone(),
        );

        let star_rx = star_tx.call_rx.take().unwrap();
        let star_tx = star_tx.call_tx;

        let global_executor = Point::global_executor()
            .to_surface()
            .with_layer(Layer::Core);
        let mut transmitter = ProtoTransmitterBuilder::new(
            Arc::new(skel.gravity_router.clone()),
            skel.exchanger.clone(),
        );
        transmitter.from = SetStrategy::Override(global_executor.clone());
        transmitter.agent = SetStrategy::Fill(Agent::HyperUser);

        let global_handler = DirectedHandlerShell::new(
            GlobalCommandExecutionHandler::new(skel.clone()),
            transmitter,
            global_executor,
            push_loc!((skel.logger, &skel.point)),
        );

        let mut forwarders = vec![];
        for (point, stub) in skel.adjacents.iter() {
            if stub.kind.is_forwarder() {
                forwarders.push(point.clone());
            }
        }

        let hyper_router = Arc::new(TxRouter::new(hyperway_endpoint.tx.clone()));
        let mut hyperway_transmitter =
            ProtoTransmitterBuilder::new(hyper_router.clone(), skel.exchanger.clone());
        hyperway_transmitter.agent = SetStrategy::Override(Agent::HyperUser);
        hyperway_transmitter.scope = SetStrategy::Override(Scope::Full);
        let hyperway_transmitter = hyperway_transmitter.build();

        let mut injector = skel
            .point()
            .clone()
            .push("injector")
            .unwrap()
            .to_surface()
            .with_layer(Layer::Gravity);

        let (to_gravity_traversal_tx, mut to_gravity_traversal_rx): (
            mpsc::Sender<Traversal<Wave>>,
            mpsc::Receiver<Traversal<Wave>>,
        ) = mpsc::channel(1024);
        {
            let skel = skel.clone();
            tokio::spawn(async move {
                while let Some(traversal) = to_gravity_traversal_rx.recv().await {
                    skel.gravity_tx.send(traversal.payload).await;
                }
            });
        }

        let layer_traversal_engine = LayerTraversalEngine::new(
            skel.clone(),
            injector.clone(),
            skel.drivers_traversal_tx.clone(),
            to_gravity_traversal_tx,
        );

        let gravity = skel.point.clone().to_surface().with_layer(Layer::Gravity);

        // relay from hyper_rx
        {
            let star_tx = star_tx.clone();
            let skel = skel.clone();
            tokio::spawn(async move {
                while let Some(wave) = hyperway_endpoint.rx.recv().await {
                    star_tx
                        .send(HyperStarCall::FromHyperway { wave, rtn: None })
                        .await;
                }
                skel.status_tx.send(Status::Panic).await.unwrap_or_default();
            });
        }

        {
            let skel = skel.clone();
            let mut drivers = drivers.clone();
            let status_tx = skel.status_tx.clone();
            let api = skel.api.clone();
            tokio::spawn(async move {
                let mut previous = DriverStatus::Unknown;
                loop {
                    if previous != drivers.status() {
                        previous = drivers.status();
                        match drivers.status() {
                            DriverStatus::Unknown => {
                                status_tx.send(Status::Unknown).await;
                            }
                            DriverStatus::Pending => {
                                status_tx.send(Status::Pending).await;
                            }
                            DriverStatus::Init => {
                                status_tx.send(Status::Init).await;
                            }
                            DriverStatus::Ready => {
                                let star_driver = skel
                                    .drivers
                                    .get(&Kind::Star(skel.kind.clone()))
                                    .await
                                    .unwrap();
                                star_driver.init_item(skel.point.to_point()).await;
                                api.start_wrangling().await;
                                // seeing if wrangling can wait on MachineApi...
                                status_tx.send(Status::Ready).await;
                            }
                            DriverStatus::Retrying(_) => {
                                status_tx.send(Status::Panic).await;
                            }
                            DriverStatus::Fatal(_) => {
                                status_tx.send(Status::Fatal).await;
                            }
                        }
                    }
                    match drivers.status_changed().await {
                        Ok(_) => {}
                        Err(_) => {
                            break;
                        }
                    }
                }
            });
        }

        let status_rx = skel.status_rx.clone();

        {
            let skel = skel.clone();
            tokio::spawn(async move {
                let logger = push_mark!(skel.logger);
                for con in &skel.template.connections {
                    if let StarCon::Connector(stub) = con {
                        match interchange
                            .mount(
                                HyperwayStub::new(
                                    stub.key.to_point().to_surface().with_layer(Layer::Gravity),
                                    Agent::HyperUser,
                                ),
                                None,
                            )
                            .await
                        {
                            Ok(local_endpoint) => {
                                match skel
                                    .machine_api
                                    .endpoint_factory(skel.key.clone(), stub.key.clone())
                                    .await
                                {
                                    Ok(remote_factory) => {
                                        match Bridge::new(
                                            local_endpoint,
                                            remote_factory,
                                            push_mark!(logger),
                                        ) {
                                            Ok(_) => {}
                                            Err(err) => {
                                                skel.logger.error(format!("could not create Bridge for remote connection: {} because {}", stub.key.to_string(), err.to_string()));
                                                skel.status_tx.send(Status::Fatal).await;
                                            }
                                        }
                                    }
                                    Err(err) => {
                                        skel.logger.error(format!("could not create endpoint factory for remote connection: {} because {}", stub.key.to_string(), err.to_string()));
                                        skel.status_tx.send(Status::Fatal).await;
                                    }
                                }
                            }
                            Err(err) => {
                                skel.logger.error(format!(
                                    "could not mount local connection: {} because {}",
                                    stub.key.to_string(),
                                    err.to_string()
                                ));
                                skel.status_tx.send(Status::Fatal).await;
                            }
                        }
                    }
                }
            });
        }

        let kind = skel.kind.clone();
        {
            let star = Self {
                skel,
                star_tx: star_tx.clone(),
                star_rx,
                drivers,
                injector,
                hyperway_transmitter,
                forwarders,
                gravity,
                hyper_router,
                layer_traversal_engine,
                global_handler,
            };
            star.start();
        }

        Ok(HyperStarApi::new(kind, star_tx, status_rx))
    }

    fn start(mut self) {
        tokio::spawn(async move {
            while let Some(call) = self.star_rx.recv().await {
                match call {
                    HyperStarCall::Init => {
                        self.drivers.init().await;
                    }
                    HyperStarCall::FromHyperway { wave, rtn } => {
                        let result = self
                            .from_hyperway(wave)
                            .await
                            .map_err(|e| SpaceErr::to_space_err(e));
                        if let Some(tx) = rtn {
                            tx.send(result);
                        } else {
                            match result {
                                Ok(_) => {}
                                Err(e) => {
                                    self.skel.err(&e.into());
                                }
                            }
                        }
                    }
                    HyperStarCall::TraverseToNextLayer(traversal) => {
                        let layer_traversal_engine = self.layer_traversal_engine.clone();
                        tokio::spawn(async move {
                            layer_traversal_engine
                                .traverse_to_next_layer(traversal)
                                .await;
                        });
                    }
                    HyperStarCall::LayerTraversalInjection(inject) => {
                        let layer_traversal_engine = self.layer_traversal_engine.clone();
                        tokio::spawn(async move {
                            layer_traversal_engine.inject(inject).await;
                        });
                    }
                    HyperStarCall::Stub(rtn) => {
                        rtn.send(self.skel.stub());
                    }
                    HyperStarCall::ToDriver(traversal) => {
                        self.drivers.visit(traversal).await;
                    }
                    HyperStarCall::ToGravity(wave) => match self.to_gravity(wave).await {
                        Ok(_) => {}
                        Err(err) => {
                            self.skel.err(&err);
                        }
                    },
                    HyperStarCall::ToHyperway(wave) => match self.to_hyperway(wave).await {
                        Ok(_) => {}
                        Err(err) => {
                            self.skel.err(&err);
                        }
                    },
                    #[cfg(test)]
                    HyperStarCall::GetSkel(rtn) => {
                        rtn.send(self.skel.clone());
                    }
                    HyperStarCall::CreateStates { point, rtn } => {
                        self.create_states(point).await;
                        rtn.send(());
                    }
                    HyperStarCall::Shard(wave) => {
                        self.shard(wave).await;
                    }
                    HyperStarCall::Wrangle(rtn) => {
                        self.wrangle(rtn).await;
                    }
                    HyperStarCall::Bounce { key, rtn } => {
                        self.bounce(key, rtn).await;
                    }
                    HyperStarCall::StartWrangling => {
                        self.start_wrangling().await;
                    }
                }
            }
        });
    }

    pub fn err(&self, err: &StarErr) {
        self.skel.logger.error(err)
    }

    async fn create_states(&self, point: Point) {
        self.skel.state.create_shell(point.clone());
    }

    async fn init_drivers(&self) {
        self.drivers.init().await;
    }

    /// receive a wave from the hyperlane... this wave should always be
    /// a Wave<Signal> of the SysMethod<Hop> which should in turn contain a SysMethod<Transport> Signal
    /// all messages are then traversed to the Star Core where they are unwrapped and then sent to
    /// gravity to start a new traversal
    #[track_caller]
    async fn from_hyperway(&self, wave: Wave) -> Result<(), StarErr> {
        self.skel
            .logger
            .track(&wave, || Tracker::new("from_hyperway", "Receive"));
        #[cfg(test)]
        {
            let wave = wave.clone();
            self.skel.diagnostic_interceptors.from_hyperway.send(wave);
        }

        let mut transport = wave.unwrap_from_hop()?;
        transport.inc_hops();
        if transport.hops > 255 {
            self.skel.logger.track_msg(
                &transport,
                || Tracker::new("from_hyperway", "HopsExceeded"),
                || "transport hops exceeded",
            );
            Err(StarErr::TransportSignalExceededMaxHops)?
        }

        if transport.to.point == self.skel.point {
            // we are now going to send this transport down the layers to the StarCore
            // where it's contents will be unwrapped from transport and routed to the appropriate particle
            let layer_engine = self.layer_traversal_engine.clone();
            let injector = self.injector.clone();

            self.skel.logger.track(&transport, || {
                Tracker::new("from_hyperway", "SendToStartLayerTraversal")
            });

            tokio::spawn(async move {
                let injection = TraversalInjection {
                    surface: injector.clone(),
                    wave: transport.to_wave(),
                    from_gravity: true,
                    dir: None,
                };
                layer_engine.inject(injection).await;
            });
            Ok(())
        } else {
            self.forward(transport).await
        }
    }

    // send this transport signal towards it's destination
    async fn forward(&self, transport: WaveVariantDef<SignalCore>) -> Result<(), StarErr> {
        if self.skel.kind.is_forwarder() {
            self.to_hyperway(transport).await
        } else {
            Err(StarErr::AttemptToForwardATransportOnANonForwardingStar)?
        }
    }
    // sending a wave that is from and to a particle into the fabric...
    // here it will be wrapped into a transport for star to star delivery or
    // sent to GLOBAL::registry if addressed in such a way
    #[track_caller]
    async fn to_gravity(&self, mut wave: Wave) -> Result<(), StarErr> {
        wave.add_to_history(self.skel.point.clone());

        #[cfg(test)]
        self.skel
            .diagnostic_interceptors
            .to_gravity
            .send(wave.clone())
            .unwrap_or_default();
        let logger = push_mark!(self.skel.logger);
        logger.track(&wave, || Tracker::new("to_gravity", "Receive"));
        if wave.is_directed()
            && wave.to().is_single()
            && wave.to().to_single().unwrap().point == *GLOBAL_EXEC
        {
            let wave = wave.to_directed().unwrap();
            let handler = self.global_handler.clone();
            tokio::spawn(async move {
                handler.handle(wave).await;
            });
            return Ok(());
        } else {
            logger
                .result(self.star_tx.send(HyperStarCall::Shard(wave)).await)
                .unwrap_or_default();
        }
        Ok(())
    }

    #[track_caller]
    async fn shard(&self, mut wave: Wave) {
        let skel = self.skel.clone();
        let locator = SmartLocator::new(self.skel.clone());
        let gravity = self.gravity.clone();
        let logger = push_mark!(self.skel.logger);
        tokio::spawn(async move {
            async fn shard(
                mut wave: Wave,
                skel: HyperStarSkel,
                locator: SmartLocator,
                gravity: Surface,
            ) -> Result<(), StarErr> {
                if wave.track() {
                    println!("\tsharding wave...{}", wave.kind().to_string());
                }
                match &mut wave {
                    Wave::Ripple(ripple) => {
                        let mut map =
                            shard_ripple_by_location(ripple, &skel.adjacents, &skel.registry)
                                .await?;
                        if ripple.track {
                            println!("\tRipple sharded into: {}", map.len());
                        }
                        for (star, mut wave) in map {
                            // add this star to history
                            wave.history.insert(skel.point.clone());
                            if star == skel.point {
                                let mut inject = TraversalInjection::new(
                                    skel.point.to_surface().with_layer(Layer::Gravity),
                                    wave.to_wave(),
                                );
                                inject.from_gravity = true;
                                if ripple.track {
                                    println!("\tripple injecting from_gravity");
                                }
                                skel.inject_tx.send(inject).await;
                            } else {
                                let mut transport = wave.to_wave().wrap_in_transport(
                                    gravity.clone(),
                                    star.to_surface().with_layer(Layer::Core),
                                );
                                transport.from(skel.point.clone().to_surface());
                                let transport = transport.build()?;
                                let transport = transport.to_signal()?;
                                skel.api.to_hyperway(transport).await;
                            }
                        }
                    }
                    _ => {
                        if wave.track() {
                            println!(
                                "sharding {} to {}",
                                wave.kind().to_string(),
                                wave.to().to_string()
                            );
                        }
                        let to = wave.to().unwrap_single();
                        let location = locator.locate(&to.point).await?;

                        if location.star.is_some() && *location.star.as_ref().unwrap() == skel.point
                        {
                            if wave.track() {
                                println!(
                                    "\tSAME POINT -> {} to {}",
                                    wave.kind().to_string(),
                                    wave.to().to_string()
                                );
                            }
                            let mut inject = TraversalInjection::new(
                                skel.point.to_surface().with_layer(Layer::Gravity),
                                wave,
                            );
                            inject.from_gravity = true;
                            skel.inject_tx.send(inject).await;
                        } else {
                            let mut transport = wave.wrap_in_transport(
                                gravity,
                                location
                                    .star
                                    .as_ref()
                                    .unwrap()
                                    .to_surface()
                                    .with_layer(Layer::Core),
                            );
                            transport.from(skel.point.clone().to_surface());
                            let transport = transport.build()?;
                            let transport = transport.to_signal()?;
                            skel.api.to_hyperway(transport).await;
                        }
                    }
                }
                Ok(())
            }
            logger
                .result(shard(wave, skel, locator, gravity).await)
                .unwrap_or_default();
        });
    }

    // send this transport signal into the hyperway
    // wrap the transport into a hop to go to one and only one star

    #[track_caller]
    async fn to_hyperway(&self, transport: WaveVariantDef<SignalCore>) -> Result<(), StarErr> {
        let logger = push_mark!(self.skel.logger);
        if self.skel.point == transport.to.point {
            // it's a bit of a strange case, but even if this star is sending a transport message
            // to itself, it still makes use of the Hyperway Interchange, which will bounce it back
            // The reason for this is that it is the Hyperway that handles things like Priority, Urgency
            // and hopefully in the future durability, whereas within the star itself all waves are
            // treated equally.
            logger.result(
                self.hyperway_transmitter
                    .direct(
                        transport.wrap_in_hop(
                            self.gravity.clone(),
                            self.skel.point.clone().to_surface(),
                        ),
                    )
                    .await,
            )?;
            Ok(())
        } else if self.skel.adjacents.contains_key(&transport.to.point) {
            let to = transport.to.clone();
            logger.result(
                self.hyperway_transmitter
                    .direct(transport.wrap_in_hop(self.gravity.clone(), to))
                    .await,
            )?;
            Ok(())
        } else if self.forwarders.len() == 1 {
            let to = self.forwarders.first().unwrap().clone().to_surface();
            logger.result(
                self.hyperway_transmitter
                    .direct(transport.wrap_in_hop(self.gravity.clone(), to))
                    .await,
            )?;
            Ok(())
        } else if self.forwarders.is_empty() {
            Err(StarErr::MissingAdjacentForwarder)?
        } else {
            unimplemented!("need to now send out a ripple search for the star being transported to")
        }
    }

    async fn start_wrangling(&self) {
        self.skel.machine_api.wait_ready().await;

        let skel = self.skel.clone();
        tokio::spawn(async move {
            let mut retries = 0;
            loop {
                match tokio::time::timeout(Duration::from_secs(60), skel.api.wrangle()).await {
                    Ok(Ok(_)) => {
                        break;
                    }
                    Ok(Err(err)) => {
                        skel.logger.error(format!(
                            "HyperStar Auto Wrangle failed: {}",
                            err.to_string()
                        ));
                    }
                    Err(err) => {
                        skel.logger.error(format!(
                            "HyperStar Auto Wrangle failed: {}",
                            err.to_string()
                        ));
                    }
                }
                skel.logger.info("trying wrangle again in 5 seconds...");
                if retries > 10 {
                    tokio::time::sleep(Duration::from_secs(15)).await;
                } else if retries > 2 {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                } else {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
                retries = retries + 1;
            }
        });
    }

    async fn wrangle(&self, rtn: oneshot::Sender<Result<StarWrangles, SpaceErr>>) {
        let skel = self.skel.clone();
        tokio::spawn(async move {
            let mut wrangler = Wrangler::new(skel.clone(), Search::Kinds);
            let mut history = HashSet::new();
            history.insert(skel.point.clone());
            wrangler.history(history);

            let discoveries = match skel.logger.result(wrangler.wrangle(false).await) {
                Ok(discoveries) => discoveries,
                Err(err) => {
                    rtn.send(Err(err)).unwrap_or_default();
                    return;
                }
            };
            let mut coalated = vec![];
            for discovery in discoveries.iter() {
                coalated.push(StarDiscovery::new(
                    StarPair::new(
                        skel.key.clone(),
                        StarKey::try_from(discovery.star_key.to_point())
                            .expect("expected star key"),
                    ),
                    discovery.clone(),
                ));
            }
            coalated.sort();
            skel.wrangles.add(coalated).await;
            rtn.send(Ok(skel.wrangles.clone())).unwrap_or_default();

            skel.status_tx.send(Status::Ready).await;
        });
    }

    async fn bounce(&self, key: StarKey, rtn: oneshot::Sender<Result<(), SpaceErr>>) {
        let transmitter = self.skel.star_transmitter.clone();
        let logger = self.skel.logger.clone();
        tokio::spawn(async move {
            let mut proto = DirectedProto::ping();
            proto.method(CmdMethod::Bounce);
            proto.to(key.to_point().to_surface().with_layer(Layer::Core));
            let pong: WaveVariantDef<PongCore> = match transmitter.direct(proto).await {
                Ok(pong) => pong,
                Err(err) => {
                    rtn.send(Err(err));
                    return;
                }
            };

            if pong.core.status.is_success() {
                rtn.send(Ok(()));
            } else {
                rtn.send(Err(pong.core.to_err()));
            }
        });
    }
}

#[derive(Clone)]
pub struct LayerTraversalEngine {
    pub skel: HyperStarSkel,
    pub injector: Surface,
    pub exit_up: mpsc::Sender<Traversal<Wave>>,
    pub exit_down: mpsc::Sender<Traversal<Wave>>,
    pub layers: HashSet<Layer>,
}

impl LayerTraversalEngine {
    pub fn new(
        skel: HyperStarSkel,
        injector: Surface,
        exit_down: mpsc::Sender<Traversal<Wave>>,
        exit_up: mpsc::Sender<Traversal<Wave>>,
    ) -> Self {
        let mut layers = HashSet::new();
        layers.insert(Layer::Field);
        layers.insert(Layer::Shell);
        Self {
            skel,
            injector,
            exit_down,
            exit_up,
            layers,
        }
    }

    async fn inject(&self, injection: TraversalInjection) {
        if injection.wave.is_directed() {
            let reflection = injection.wave.clone().to_directed().unwrap().reflection();
            let surface = injection.surface.clone();
            match self.start_layer_traversal(injection).await {
                Ok(_) => {}
                Err(err) => {
                    // if it can be reflected then send back as an error
                    match reflection {
                        Ok(reflection) => {
                            let err = SpaceErr::to_space_err(err);
                            let reflect = reflection.make(err.into(), self.skel.point.to_surface());
                            let injection = TraversalInjection {
                                surface,
                                wave: reflect.to_wave(),
                                from_gravity: false,
                                dir: None,
                            };
                            self.skel
                                .logger
                                .result(self.start_layer_traversal(injection).await)
                                .unwrap_or_default();
                        }
                        Err(err) => {
                            self.skel.logger.error(err.to_string());
                        }
                    }
                }
            }
        } else {
            self.skel
                .logger
                .result(self.start_layer_traversal(injection).await)
                .unwrap_or_default();
        }
    }

    async fn start_layer_traversal(&self, injection: TraversalInjection) -> Result<(), StarErr> {
        let wave = injection.wave;
        let from_gravity = injection.from_gravity;
        let inject_dir = injection.dir;
        let injection = injection.surface;

        #[cfg(test)]
        self.skel
            .diagnostic_interceptors
            .start_layer_traversal_wave
            .send(wave.clone())
            .unwrap_or_default();

        let logger = push_mark!(self.skel.logger);

        let tos = match wave.kind() {
            WaveKind::Ripple => {
                let mut tos = vec![];
                match wave.to() {
                    Recipients::Single(single) => {
                        tos.push(single);
                    }
                    Recipients::Multi(ports) => {
                        for port in &ports {
                            let record = self.skel.registry.record(&port.point).await?;
                            let loc = logger.result(
                                record
                                    .location
                                    .star
                                    .ok_or(StarErr::UnprovisionedMultiPortRipple),
                            )?;
                            if loc == self.skel.point {
                                tos.push(port.clone());
                            }
                        }
                    }
                    Recipients::Watchers(_) => {}
                    Recipients::Stars => {
                        if self.skel.point == wave.from().point {
                            tos.push(self.skel.point.to_surface().with_layer(Layer::Gravity));
                        } else {
                            tos.push(self.skel.point.to_surface().with_layer(Layer::Core));
                        }
                    }
                }
                tos
            }
            _ => {
                vec![wave.to().unwrap_single()]
            }
        };

        for to in tos {
            let record = match self.skel.registry.record(&to.point).await {
                Ok(record) => record,
                Err(_) => {
                    let err = SpaceErr::not_found(format!(
                        "could not locate record for surface {} from {}",
                        to.to_string(),
                        wave.from().to_string()
                    ));
                    return Err(err)?;
                }
            };
            let plan = record.details.stub.kind.wave_traversal_plan().clone();

            let mut dest = None;
            let mut dir = TraversalDirection::Core;

            // injection direction can be forced when we are sending a message from core
            // to the same core
            if let Some(inject_dir) = &inject_dir {
                dir = inject_dir.clone();
            }
            // now we check if we are doing an inter point delivery (from one layer to another in the same Particle)
            // if this delivery was from_hyperway, then it was certainly a message being routed back to the star
            // and is not considered an inter point delivery
            else if from_gravity {
                dir = TraversalDirection::Core;
                dest.replace(to.layer.clone());
                if wave.track() {
                    println!("\twave is from_gravity so it gets a Core direction")
                }
            } else if to.point.is_global() {
                dir = TraversalDirection::Fabric;
                if wave.track() {
                    println!("\twave is a GLOBAL point so it gets a Fabric Direction")
                }
            } else if to.point == wave.from().point {
                if wave.track() {
                    println!(
                        "\twave is to and from the same point... {} <{}>",
                        wave.from().to_string(),
                        wave.kind().to_string()
                    )
                }
                // it's the SAME point, so the to layer becomes our dest
                dest.replace(to.layer.clone());

                // make sure we have this layer in the plan
                if to.layer != Layer::Gravity && !plan.has_layer(&to.layer) {
                    Err(StarErr::TraversalPlanNotFound {
                        wave: wave.id(),
                        layer: to.layer.clone(),
                        kind: record.details.stub.kind.clone(),
                    })?;
                }

                // dir is from inject_layer to dest
                dir = match TraversalDirection::new(&injection.layer, &to.layer) {
                    Ok(dir) => dir,
                    Err(_) => {
                        // looks like we are already on the dest layer...
                        // that means it doesn't matter what the TraversalDirection is
                        TraversalDirection::Fabric
                    }
                };
                if wave.track() {
                    println!("\t...decided wave is in the {} Direction", dir.to_string())
                }
            } else {
                if wave.track() {
                    println!(
                        "\tno matches so choosing Fabric Direction ({})",
                        self.skel.point.to_string()
                    );
                }
                dir = TraversalDirection::Fabric;
            }
            if wave.track() {
                if dest.is_some() {
                    println!(
                        "\t{} -> {} DIR: {} dest: {}",
                        wave.from().to_string(),
                        to.to_string(),
                        dir.to_string(),
                        dest.as_ref().unwrap().to_string()
                    );
                } else {
                    println!(
                        "\t{} -> {} DIR: {}",
                        wave.from().to_string(),
                        to.to_string(),
                        dir.to_string()
                    );
                }
            }

            let traversal_logger = push_loc!((self.skel.logger, &to));
            let traversal_logger = log_span!(traversal_logger);

            let point = if dir == TraversalDirection::Core {
                to.clone().to_point()
            } else {
                // if injected by any other point then the injector is the point that this traversal belongs to
                wave.from().to_point()
            };

            let mut traversal = Traversal::new(
                wave.clone(),
                record,
                injection.layer.clone(),
                traversal_logger,
                dir,
                dest,
                to,
                point,
            );

            // in the case that we injected into a layer that is not part
            // of this plan, we need to send the traversal to the next layer
            if !self.has_layer(&traversal.layer) {
                match traversal.next() {
                    None => {
                        self.exit(traversal).await;
                        continue;
                    }
                    Some(_) => {}
                }
            }

            #[cfg(test)]
            self.skel
                .diagnostic_interceptors
                .start_layer_traversal
                .send(traversal.clone())
                .unwrap_or_default();

            // alright, let's visit the injection layer first...
            self.visit_layer(traversal).await?;
        }
        Ok(())
    }

    fn has_layer(&self, layer: &Layer) -> bool {
        *layer == Layer::Shell || *layer == Layer::Field
    }

    async fn exit(&self, traversal: Traversal<Wave>) -> Result<(), SpaceErr> {
        match traversal.dir {
            TraversalDirection::Fabric => {
                if traversal.track() {
                    println!("\tEXIT UP")
                }
                self.exit_up.send(traversal).await;
                return Ok(());
            }
            TraversalDirection::Core => {
                if traversal.track() {
                    println!("\tEXIT DOWN")
                }
                self.exit_down.send(traversal).await;
                return Ok(());
            }
        }
    }

    async fn visit_layer(&self, traversal: Traversal<Wave>) -> Result<(), SpaceErr> {
        let logger = push_mark!(self.skel.logger);
        logger.track(&traversal, || {
            Tracker::new(
                format!("visit:layer@{}", traversal.layer.to_string()),
                "Visit",
            )
        });

        match traversal.layer {
            Layer::Field => {
                let field = Field::new(traversal.point.clone(), self.skel.clone());
                let logger = push_loc!((
                    self.skel.logger,
                    self.skel.point.clone().into_surface(Layer::Field)
                ));
                tokio::spawn(async move {
                    logger
                        .result(field.visit(traversal).await)
                        .unwrap_or_default();
                });
            }
            Layer::Shell => {
                let shell = Shell::new(
                    self.skel.clone(),
                    self.skel
                        .state
                        .find_shell(&traversal.point.to_surface().with_layer(Layer::Shell))?,
                );

                let logger = push_loc!((
                    self.skel.logger,
                    self.skel.point.clone().into_surface(Layer::Shell)
                ));
                tokio::spawn(async move {
                    logger
                        .result(shell.visit(traversal).await)
                        .unwrap_or_default();
                });
            }
            _ => {
                logger
                    .result(self.exit(traversal).await)
                    .unwrap_or_default();
            }
        }
        Ok(())
    }

    async fn traverse_to_next_layer(&self, mut traversal: Traversal<Wave>) {
        let logger = push_mark!(self.skel.logger);

        self.skel
            .logger
            .track(&traversal, || Tracker::new("traverse", "NextLayer"));

        if traversal.dest.is_some() && traversal.layer == *traversal.dest.as_ref().unwrap() {
            self.visit_layer(traversal).await;
            return;
        }

        let next = traversal.next();

        match next {
            None => match traversal.dir {
                TraversalDirection::Fabric => {
                    self.exit_up.send(traversal).await;
                }
                TraversalDirection::Core => {
                    logger.warn("should not have traversed a wave all the way to the core in Star");
                }
            },
            Some(_) => {
                logger
                    .result(self.visit_layer(traversal).await)
                    .unwrap_or_default();
            }
        }
    }
}

pub struct StarMount {
    pub point: Point,
    pub kind: MountKind,
    pub tx: mpsc::Sender<Wave>,
}

#[derive(Clone)]
pub struct LayerInjectionRouter {
    pub inject_tx: mpsc::Sender<TraversalInjection>,
    pub injector: Surface,
    pub direction: Option<TraversalDirection>,
}

impl LayerInjectionRouter {
    pub fn new(skel: HyperStarSkel, injector: Surface) -> Self {
        Self {
            inject_tx: skel.inject_tx.clone(),
            injector,
            direction: None,
        }
    }

    pub fn with(&self, injector: Surface) -> Self {
        Self {
            inject_tx: self.inject_tx.clone(),
            injector,
            direction: None,
        }
    }

    pub fn injector(inject_tx: mpsc::Sender<TraversalInjection>, injector: Surface) -> Self {
        Self {
            inject_tx,
            injector,
            direction: None,
        }
    }
}

#[derive(Clone)]
pub struct TraverseToNextRouter {
    pub tx: mpsc::Sender<Traversal<Wave>>,
}

impl TraverseToNextRouter {
    pub fn new(tx: mpsc::Sender<Traversal<Wave>>) -> Self {
        Self { tx }
    }
}

#[async_trait]
impl TraversalRouter for TraverseToNextRouter {
    async fn traverse(&self, traversal: Traversal<Wave>) -> Result<(), SpaceErr> {
        self.tx.send(traversal).await?;
        Ok(())
    }
}

#[async_trait]
impl Router for LayerInjectionRouter {
    async fn route(&self, wave: Wave) {
        let mut inject = TraversalInjection::new(self.injector.clone(), wave);
        inject.dir = self.direction.clone();
        self.inject_tx.send(inject).await;
    }
}

pub trait TopicHandler: Send + Sync + DirectedHandler {
    fn source_selector(&self) -> &SurfaceSelector;
}

pub trait TopicHandlerSerde<T: TopicHandler> {
    fn serialize(&self, handler: T) -> Substance;
    fn deserialize(&self, ser: Substance) -> T;
}

#[derive(Clone)]
pub struct StarTemplate {
    pub key: StarKey,
    pub kind: StarSub,
    pub connections: Vec<StarCon>,
}

impl StarTemplate {
    pub fn new(key: StarKey, kind: StarSub) -> Self {
        Self {
            key,
            kind,
            connections: vec![],
            //            services: Templates::default(),
        }
    }

    pub fn to_stub(&self) -> StarStub {
        StarStub::new(self.key.clone(), self.kind.clone())
    }

    pub fn receive(&mut self, stub: StarStub) {
        self.connections.push(StarCon::Receiver(stub));
    }

    pub fn connect(&mut self, stub: StarStub) {
        self.connections.push(StarCon::Connector(stub));
    }
}

#[derive(Clone)]
pub enum StarCon {
    Receiver(StarStub),
    Connector(StarStub),
}

impl StarCon {
    pub fn is_connector(&self) -> bool {
        match self {
            StarCon::Receiver(_) => false,
            StarCon::Connector(_) => true,
        }
    }

    pub fn is_receiver(&self) -> bool {
        match self {
            StarCon::Receiver(_) => true,
            StarCon::Connector(_) => false,
        }
    }

    pub fn stub(&self) -> &StarStub {
        match self {
            StarCon::Receiver(stub) => stub,
            StarCon::Connector(stub) => stub,
        }
    }

    pub fn key(&self) -> &StarKey {
        match self {
            StarCon::Receiver(stub) => &stub.key,
            StarCon::Connector(stub) => &stub.key,
        }
    }

    pub fn kind(&self) -> &StarSub {
        match self {
            StarCon::Receiver(stub) => &stub.kind,
            StarCon::Connector(stub) => &stub.kind,
        }
    }
}

async fn shard_ripple_by_location(
    ripple: &WaveVariantDef<Ripple>,
    adjacent: &HashMap<Point, StarStub>,
    registry: &Registry,
) -> Result<HashMap<Point, WaveVariantDef<Ripple>>, StarErr> {
    let mut map = HashMap::new();
    for (star, recipients) in shard_by_location(ripple.to.clone(), adjacent, registry).await? {
        if !ripple.history.contains(&star) {
            let mut ripple = ripple.clone();
            ripple.variant.to = recipients;
            map.insert(star, ripple);
        } else {
        }
    }
    Ok(map)
}

pub async fn ripple_to_singulars<E>(
    ripple: WaveVariantDef<Ripple>,
    adjacent: &HashSet<Point>,
    registry: &Registry,
) -> Result<Vec<WaveVariantDef<SingularRipple>>, StarErr> {
    let mut rtn = vec![];
    for port in to_ports(ripple.to.clone(), adjacent, registry).await? {
        let wave = ripple.as_single(port);
        rtn.push(wave)
    }
    Ok(rtn)
}

pub async fn shard_by_location(
    recipients: Recipients,
    adjacent: &HashMap<Point, StarStub>,
    registry: &Registry,
) -> Result<HashMap<Point, Recipients>, StarErr> {
    match recipients {
        Recipients::Single(single) => {
            unimplemented!()
            /*
            let mut map = HashMap::new();
            let record = registry.locate(&single.point).await?;
            map.insert(record.location, Recipients::Single(single));
            Ok(map)

             */
        }
        Recipients::Multi(multi) => {
            unimplemented!()
            /*
            let mut map: HashMap<Point, Vec<Port>> = HashMap::new();
            for p in multi {
                let record = registry.locate(&p).await?;
                if let Some(found) = map.get_mut(&record.location) {
                    found.push(p);
                } else {
                    map.insert(record.location, vec![p]);
                }
            }


            let mut map2 = HashMap::new();
            for (location, points) in map {
                map2.insert(location, Recipients::Multi(points));
            }
            Ok(map2)
             */
        }
        Recipients::Watchers(_) => {
            let mut map = HashMap::new();
            // todo
            Ok(map)
        }
        Recipients::Stars => {
            let mut map = HashMap::new();
            for (star, _) in adjacent {
                map.insert(star.clone(), Recipients::Stars);
            }
            Ok(map)
        }
    }
}

pub async fn to_ports(
    recipients: Recipients,
    adjacent: &HashSet<Point>,
    registry: &Registry,
) -> Result<Vec<Surface>, StarErr> {
    match recipients {
        Recipients::Single(single) => Ok(vec![single]),
        Recipients::Multi(multi) => Ok(multi.into_iter().map(|p| p).collect()),
        Recipients::Watchers(watch) => {
            unimplemented!();
        }
        Recipients::Stars => {
            let stars: Vec<Surface> = adjacent
                .clone()
                .into_iter()
                .map(|p| p.to_surface())
                .collect();
            Ok(stars)
        }
    }
}

#[derive(Clone)]
pub struct DiagnosticInterceptors {
    pub from_hyperway: broadcast::Sender<Wave>,
    pub to_gravity: broadcast::Sender<Wave>,
    pub to_hyperway: broadcast::Sender<WaveVariantDef<SignalCore>>,
    pub start_layer_traversal_wave: broadcast::Sender<Wave>,
    pub start_layer_traversal: broadcast::Sender<Traversal<Wave>>,
    pub transport_endpoint: broadcast::Sender<Wave>,
    pub reflected_endpoint: broadcast::Sender<Wave>,
    pub assignment: broadcast::Sender<Assign>,
    pub err: broadcast::Sender<String>,
}

impl DiagnosticInterceptors {
    pub fn new() -> Self {
        let (from_hyperway, _) = broadcast::channel(1024);
        let (to_hyperway, _) = broadcast::channel(1024);
        let (to_gravity, _) = broadcast::channel(1024);
        let (start_layer_traversal, _) = broadcast::channel(1024);
        let (start_layer_traversal_wave, _) = broadcast::channel(1024);
        let (err, _) = broadcast::channel(1024);
        let (transport_endpoint, _) = broadcast::channel(1024);
        let (reflected_endpoint, _) = broadcast::channel(1024);
        let (assignment, _) = broadcast::channel(1024);
        Self {
            from_hyperway,
            to_hyperway,
            to_gravity,
            start_layer_traversal,
            start_layer_traversal_wave,
            err,
            transport_endpoint,
            reflected_endpoint,
            assignment,
        }
    }
}

#[derive(Clone)]
pub struct SmartLocator {
    pub skel: HyperStarSkel,
}

impl SmartLocator {
    pub fn new(skel: HyperStarSkel) -> Self {
        Self { skel }
    }

    pub async fn locate(&self, point: &Point) -> Result<ParticleLocation, StarErr> {
        let record = self.skel.registry.record(&point).await?;
        match &record.location.star {
            Some(_) => Ok(record.location),
            None => {
                // now we must provision
                self.provision(point, StateSrc::None).await
            }
        }
    }

    pub async fn provision(
        &self,
        point: &Point,
        state: StateSrc,
    ) -> Result<ParticleLocation, StarErr> {
        self.skel
            .logger
            .result(self.provision_inner(point, state).await)
    }

    #[async_recursion]
    async fn provision_inner(
        &self,
        point: &Point,
        state: StateSrc,
    ) -> Result<ParticleLocation, StarErr> {
        // check if parent is provisioned
        let parent = point.parent().ok_or(StarErr::ExpectedRootProvisioned)?;
        let mut parent_record = self.skel.registry.record(&parent).await?;
        if parent_record.location.star.is_none() {
            self.provision_inner(&parent, StateSrc::None).await?;
            parent_record = self.skel.registry.record(&parent).await?;
        }

        let parent_star = parent_record.location.star.unwrap();
        let provision = Provision::new(point.clone(), state);
        let mut wave = DirectedProto::ping();
        wave.method(HypMethod::Provision);
        wave.body(HyperSubstance::Provision(provision).into());
        wave.from(self.skel.point.clone().to_surface().with_layer(Layer::Core));
        wave.to(parent_star.to_surface().with_layer(Layer::Core));
        let pong: WaveVariantDef<PongCore> = self.skel.star_transmitter.direct(wave).await?;
        (pong.core.clone().body).expect(SubstanceKind::Location);
        if pong.core.status.as_u16() == 200 {
            let location = match &pong.core.body {
                Substance::Location(location) => location,
                s => Err(SpaceErr::ExpectedSubstance {
                    expected: SubstanceKind::Location,
                    found: s.kind(),
                })?,
            };

            Ok(location.clone())
        } else {
            self.skel
                .registry
                .set_status(&point, &Status::Panic)
                .await?;

            match self.skel.registry.record(&point).await {
                Ok(record) => Err(RegErr::dupe())?,
                Err(err) => Err(err)?,
            }
        }
    }
}

#[derive(Error, Debug)]
pub enum StarErr {
    #[error("caused by '{0}'")]
    SpaceErr(#[source] SpaceErr),
    #[error("cannot create_in_star in star {point} for parent point {parent} since it is not a point within this star"
    )]
    PointNotInStar { point: Point, parent: Point },
    #[error("star expected Root to be already provisioned")]
    ExpectedRootProvisioned,
    #[error("could not find parent '{parent}' caused by '{source}'")]
    CannotFindParent {
        #[source]
        source: RegErr,
        parent: Point,
    },
    #[error("transport signal exceeded maximum hops")]
    TransportSignalExceededMaxHops,
    #[error("attempt to forward a transport on a non forwarding star")]
    AttemptToForwardATransportOnANonForwardingStar,
    #[error("star needs to send a transport to a non-adjacent star yet does not have any adjacent forwarders"
    )]
    MissingAdjacentForwarder,
    #[error("attempt to send wave {wave} to layer {layer} that the recipient Kind {kind} does not have in its traversal plan"
    )]
    TraversalPlanNotFound {
        wave: WaveId,
        layer: Layer,
        kind: Kind,
    },
    #[error("multi port ripple has recipient that is not located, this should have been provisioned when the ripple was sent"
    )]
    UnprovisionedMultiPortRipple,
    #[error("could not find assign kind '{0}' to self")]
    CouldNotAssignToSelf(Kind),
    #[error("could not find a host to provision '{0}'")]
    CouldNotFindHostToProvision(Kind),
    #[error("{0}")]
    Anyhow(Arc<anyhow::Error>),
}

impl<E> From<E> for StarErr
where
    E: Sized + SpatialError,
{
    fn from(err: E) -> Self {
        Self::Anyhow(err.anyhow())
    }
}

impl CoreReflector for StarErr {
    fn as_reflected_core(self) -> ReflectedCore {
        if let StarErr::SpaceErr(err) = self {
            err.as_reflected_core()
        } else {
            ReflectedCore {
                headers: Default::default(),
                status: Default::default(),
                body: Substance::Err(SubstanceErr("from StarErr".to_string())),
            }
        }
    }
}

impl StarErr {
    pub fn point_not_in_star(point: &Point, parent: &Point) -> Self {
        let point = point.clone();
        let parent = parent.clone();
        Self::PointNotInStar { point, parent }
    }
}

#[derive(Debug, Clone, strum_macros::EnumString, strum_macros::Display)]
pub enum StarErrCtx {
    CreateInStar,
}
