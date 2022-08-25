use crate::driver::{
    Driver, DriverCtx, DriverDriver, DriverDriverFactory, DriverFactory, DriverSkel, DriverStatus,
    Drivers, DriversApi, DriversCall, HyperDriverFactory, Item, ItemDirectedHandler, ItemHandler,
    ItemSkel,
};
use crate::field::{Field, FieldState};
use crate::machine::MachineSkel;
use crate::shell::Shell;
use crate::state::ShellState;
use crate::{DriversBuilder, PlatErr, Platform, Registry, RegistryApi, SmartLocator};
use cosmic_api::bin::Bin;
use cosmic_api::cli::RawCommand;
use cosmic_api::command::command::common::StateSrc;
use cosmic_api::command::request::create::{Create, Strategy};
use cosmic_api::command::request::set::Set;
use cosmic_api::config::config::bind::{BindConfig, RouteSelector};
use cosmic_api::error::MsgErr;
use cosmic_api::id::id::{
    BaseKind, Kind, Layer, Point, Port, PortSelector, RouteSeg, Sub, ToBaseKind, ToPoint, ToPort,
    Topic, TraversalLayer, Uuid, GLOBAL_EXEC,
};
use cosmic_api::id::{StarKey, StarStub, StarSub, TraversalInjection};
use cosmic_api::id::{Traversal, TraversalDirection};
use cosmic_api::log::{PointLogger, RootLogger, Trackable, Tracker};
use cosmic_api::parse::{bind_config, route_attribute, Env};
use cosmic_api::particle::particle::{Details, Status, Stub};
use cosmic_api::quota::Timeouts;
use cosmic_api::substance::substance::{Substance, ToSubstance};
use cosmic_api::sys::{Assign, AssignmentKind, Discoveries, Discovery, Location, Search, Sys};
use cosmic_api::util::{log, ValueMatcher, ValuePattern};
use cosmic_api::wave::{Agent, Bounce, BounceBacks, CoreBounce, DirectedHandler, DirectedHandlerSelector, DirectedHandlerShell, DirectedKind, DirectedProto, DirectedWave, Echo, Echoes, Handling, HandlingKind, InCtx, Method, Ping, Pong, Priority, ProtoTransmitter, ProtoTransmitterBuilder, RecipientSelector, Recipients, Reflectable, ReflectedCore, ReflectedWave, Retries, Ripple, RootInCtx, Router, Scope, SetStrategy, Signal, SingularRipple, ToRecipients, TxRouter, WaitTime, Wave, CmdMethod};
use cosmic_api::wave::{DirectedCore, Exchanger, HyperWave, SysMethod, UltraWave};
use cosmic_api::ArtRef;
use cosmic_api::{MountKind, Registration, State, StateFactory, HYPERUSER};
use cosmic_hyperlane::{HyperClient, HyperRouter, HyperwayExt};
use dashmap::mapref::one::{Ref, RefMut};
use dashmap::DashMap;
use futures::future::{join_all, BoxFuture};
use futures::FutureExt;
use http::StatusCode;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::marker::PhantomData;
use std::ops::{Add, Deref, DerefMut};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::error::SendError;
use tokio::sync::oneshot::error::RecvError;
use tokio::sync::{broadcast, mpsc, oneshot, watch, Mutex, RwLock};
use tokio::time::error::Elapsed;
use tracing::{error, info};
use crate::global::GlobalExecutionChamber;
use crate::star::StarCall::LayerTraversalInjection;

#[derive(Clone)]
pub struct StarState<P>
where
    P: Platform + 'static,
{
    states: Arc<DashMap<Port, Arc<RwLock<dyn State>>>>,
    topic: Arc<DashMap<Port, Arc<dyn TopicHandler>>>,
    tx: mpsc::Sender<StateCall>,
    field: Arc<DashMap<Point, FieldState<P>>>,
    shell: Arc<DashMap<Point, ShellState>>,
}

impl<P> StarState<P>
where
    P: Platform + 'static,
{
    pub fn create_field(&self, point: Point) {
        self.field.insert(point.clone(), FieldState::new(point));
    }

    pub fn create_shell(&self, point: Point) {
        self.shell.insert(point.clone(), ShellState::new(point));
    }

    pub fn new() -> Self {
        let states: Arc<DashMap<Port, Arc<RwLock<dyn State>>>> = Arc::new(DashMap::new());

        let (tx, mut rx) = mpsc::channel(32 * 1024);

        {
            let states = states.clone();
            tokio::spawn(async move {
                while let Some(call) = rx.recv().await {
                    match call {
                        StateCall::Get { port, tx } => match states.get(&port) {
                            None => {
                                tx.send(Ok(None));
                            }
                            Some(state) => {
                                tx.send(Ok(Some(state.value().clone())));
                            }
                        },
                        StateCall::Put { port, state, tx } => {
                            if states.contains_key(&port) {
                                tx.send(Err(MsgErr::bad_request()));
                            } else {
                                states.insert(port, state);
                                tx.send(Ok(()));
                            }
                        }
                    }
                }
            });
        }

        Self {
            states,
            topic: Arc::new(DashMap::new()),
            field: Arc::new(DashMap::new()),
            shell: Arc::new(DashMap::new()),
            tx,
        }
    }

    pub fn api(&self) -> StateApi {
        StateApi::new(self.tx.clone())
    }

    pub fn states_tx(&self) -> mpsc::Sender<StateCall> {
        self.tx.clone()
    }

    pub fn topic_handler(&self, port: Port, handler: Arc<dyn TopicHandler>) {
        self.topic.insert(port, handler);
    }

    pub async fn find_state<S>(&self, port: &Port) -> Result<Arc<RwLock<dyn State>>, MsgErr> {
        Ok(self
            .states
            .get(port)
            .ok_or(format!("could not find state for: {}", port.to_string()))?
            .value()
            .clone())
    }

    pub fn find_topic(
        &self,
        port: &Port,
        source: &Port,
    ) -> Option<Result<Arc<dyn TopicHandler>, MsgErr>> {
        match self.topic.get(port) {
            None => None,
            Some(topic) => {
                let topic = topic.value().clone();
                if topic.source_selector().is_match(source).is_ok() {
                    Some(Ok(topic))
                } else {
                    Some(Err(MsgErr::forbidden()))
                }
            }
        }
    }

    pub fn find_field(&self, point: &Point) -> Result<FieldState<P>, MsgErr> {
        let rtn = self
            .field
            .get(point)
            .ok_or(format!(
                "expected field state for point: {}",
                point.to_string()
            ))?
            .value()
            .clone();
        Ok(rtn)
    }

    pub fn find_shell(&self, point: &Point) -> Result<ShellState, MsgErr> {
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
pub struct StarSkel<P>
where
    P: Platform + 'static,
{
    pub api: StarApi<P>,
    pub key: StarKey,
    pub point: Point,
    pub locator: SmartLocator<P>,
    pub kind: StarSub,
    pub logger: PointLogger,
    pub registry: Registry<P>,
    pub traverse_to_next_tx: mpsc::Sender<Traversal<UltraWave>>,
    pub inject_tx: mpsc::Sender<TraversalInjection>,
    pub machine: MachineSkel<P>,
    pub exchanger: Exchanger,
    pub state: StarState<P>,
    pub connections: Vec<StarCon>,
    pub adjacents: HashMap<Point, StarStub>,
    pub wrangles: StarWrangles,
    pub gravity_tx: mpsc::Sender<UltraWave>,
    pub gravity_router: TxRouter,
    pub gravity_transmitter: ProtoTransmitter,
    pub drivers: DriversApi<P>,
    pub drivers_traversal_tx: mpsc::Sender<Traversal<UltraWave>>,
    pub status_tx: mpsc::Sender<Status>,
    pub status_rx: watch::Receiver<Status>,

    #[cfg(test)]
    pub diagnostic_interceptors: DiagnosticInterceptors<P>,
}

impl<P> StarSkel<P>
where
    P: Platform,
{
    pub async fn new(
        template: StarTemplate,
        machine: MachineSkel<P>,
        star_tx: &mut StarTx<P>,
    ) -> Self {
        let point = template.key.clone().to_point();
        let logger = machine.logger.point(point.clone());
        let exchanger = Exchanger::new(point.clone().to_port(), machine.timeouts.clone());
        let state = StarState::new();
        let api = StarApi::new(
            template.kind.clone(),
            star_tx.call_tx.clone(),
            star_tx.status_rx.clone(),
        );

        let mut adjacents = HashMap::new();
        // prime the searcher by mapping the immediate lanes
        for hyperway in template.connections.clone() {
            adjacents.insert(hyperway.key().clone().to_point(), hyperway.stub().clone());
        }

        let gravity_router = TxRouter::new(star_tx.gravity_tx.clone());
        let mut gravity_transmitter =
            ProtoTransmitterBuilder::new(Arc::new(gravity_router.clone()), exchanger.clone());
        gravity_transmitter.from = SetStrategy::Override(point.clone().to_port());
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

        let locator = SmartLocator::new( machine.registry.clone() );

        Self {
            api,
            key: template.key,
            point,
            kind: template.kind,
            logger,
            locator,
            gravity_tx: star_tx.gravity_tx.clone(),
            gravity_router,
            gravity_transmitter,
            traverse_to_next_tx: star_tx.traverse_to_next_tx.clone(),
            inject_tx: star_tx.inject_tx.clone(),
            exchanger,
            state,
            connections: template.connections,
            registry: machine.registry.clone(),
            machine,
            adjacents,
            wrangles: StarWrangles::new(),
            drivers,
            drivers_traversal_tx: star_tx.drivers_traversal_tx.clone(),
            status_tx: star_tx.status_tx.clone(),
            status_rx: star_tx.status_rx.clone(),
            #[cfg(test)]
            diagnostic_interceptors: DiagnosticInterceptors::new(),
        }
    }

    /*
    pub async fn create_star_particle(&self, point: Point, kind: Kind ) -> Result<(),P::Err> {

        if !self.point.is_parent_of(&point) {
            return Err(P::Err::new(format!("create_star_particle must be a child of star. expected: {}+:**, encountered: {}", self.point.to_string(), point.to_string())));
        }

        let registration = Registration {
            point: point.clone(),
            kind,
            registry: Default::default(),
            properties: Default::default(),
            owner: self.point.clone(),
            strategy: Strategy::Override,
            status: Status::Ready
        };

        self.registry.register(&registration).await?;
        self.api.create_states(point.clone()).await?;
        self.registry.assign(&point).send(self.point.clone());
        Ok(())
    }

     */

    #[track_caller]
    pub async fn create(&self, who: &str, create: Create ) -> Result<Details,P::Err>{
        let logger = self.logger.push_mark("create").unwrap();
        let global = GlobalExecutionChamber::new(self.clone());
        let details = self.logger.result_ctx(format!("StarSkel::create_in_star(register({}))",create.template.kind.to_string()).as_str(),global.create(&create, &Agent::HyperUser).await)?;
        let assign_body = Assign::new( AssignmentKind::Create, details.clone(), StateSrc::None );
        let mut assign = DirectedProto::sys(self.point.clone().to_port().with_layer(Layer::Core), SysMethod::Assign);

        assign.body(assign_body.into());
        let router = Arc::new(LayerInjectionRouter::new(self.clone(), self.point.clone().to_port().with_layer(Layer::Shell)));
        let mut transmitter = ProtoTransmitterBuilder::new(router, self.exchanger.clone() );
        transmitter.from = SetStrategy::Override(self.point.to_port().with_layer(Layer::Core));
        transmitter.agent = SetStrategy::Override(Agent::HyperUser);
        let transmitter = transmitter.build();

        let assign_result: Wave<Pong> = logger.result_ctx("StarSkel::create(assign_result)",transmitter.direct(assign).await)?;
        let logger = logger.push_mark("result").unwrap();
        logger.result(assign_result.ok_or())?;
        Ok(details)
    }

    pub fn err<M: ToString>(&self, message: M) -> Result<(), P::Err> {
        self.logger.warn(message.to_string());
        return Err(P::Err::new(message.to_string()));
    }

    pub fn location(&self) -> &Point {
        &self.logger.point
    }

    pub fn stub(&self) -> StarStub {
        StarStub::new(self.key.clone(), self.kind.clone())
    }
}

pub enum StarCall<P>
where
    P: Platform,
{
    Init,
    CreateStates {
        point: Point,
        rtn: oneshot::Sender<()>,
    },
    Stub(oneshot::Sender<StarStub>),
    FromHyperway {
        wave: UltraWave,
        rtn: Option<oneshot::Sender<Result<(), MsgErr>>>,
    },
    TraverseToNextLayer(Traversal<UltraWave>),
    LayerTraversalInjection(TraversalInjection),
    ToDriver(Traversal<UltraWave>),
    Phantom(PhantomData<P>),
    ToGravity(UltraWave),
    Shard(UltraWave),
    Assign {
        details: Details,
        rtn: oneshot::Sender<Result<Point, MsgErr>>,
    },
    #[cfg(test)]
    GetSkel(oneshot::Sender<StarSkel<P>>),
}

pub struct StarTx<P>
where
    P: Platform,
{
    pub gravity_tx: mpsc::Sender<UltraWave>,
    pub traverse_to_next_tx: mpsc::Sender<Traversal<UltraWave>>,
    pub inject_tx: mpsc::Sender<TraversalInjection>,
    pub drivers_traversal_tx: mpsc::Sender<Traversal<UltraWave>>,
    pub call_tx: mpsc::Sender<StarCall<P>>,
    pub call_rx: Option<mpsc::Receiver<StarCall<P>>>,
    pub drivers_call_tx: mpsc::Sender<DriversCall<P>>,
    pub drivers_call_rx: Option<mpsc::Receiver<DriversCall<P>>>,
    pub drivers_status_tx: Option<watch::Sender<DriverStatus>>,
    pub drivers_status_rx: watch::Receiver<DriverStatus>,
    pub status_tx: mpsc::Sender<Status>,
    pub status_rx: watch::Receiver<Status>,
}

impl<P> StarTx<P>
where
    P: Platform,
{
    pub fn new(point: Point) -> Self {
        let (gravity_tx, mut gravity_rx) = mpsc::channel(1024);
        let (inject_tx, mut inject_rx) = mpsc::channel(1024);
        let (traverse_to_next_tx, mut traverse_to_next_rx): (
            mpsc::Sender<Traversal<UltraWave>>,
            mpsc::Receiver<Traversal<UltraWave>>,
        ) = mpsc::channel(1024);
        let (drivers_traversal_tx, mut drivers_rx) = mpsc::channel(1024);
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
                    call_tx.send(StarCall::ToGravity(wave)).await;
                }
            });
        }

        {
            let call_tx = call_tx.clone();
            tokio::spawn(async move {
                while let Some(traversal) = traverse_to_next_rx.recv().await {
                    match call_tx
                        .send(StarCall::TraverseToNextLayer(traversal.clone()))
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
                        .send(StarCall::LayerTraversalInjection(inject))
                        .await;
                }
            });
        }

        {
            let call_tx = call_tx.clone();
            tokio::spawn(async move {
                while let Some(inject) = drivers_rx.recv().await {
                    match call_tx.send(StarCall::ToDriver(inject)).await {
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

    pub fn star_rx(&mut self) -> Option<mpsc::Receiver<StarCall<P>>> {
        self.call_rx.take()
    }
}

#[derive(Clone)]
pub struct StarApi<P>
where
    P: Platform,
{
    pub kind: StarSub,
    tx: mpsc::Sender<StarCall<P>>,
    pub status_rx: watch::Receiver<Status>,
}

impl<P> StarApi<P>
where
    P: Platform,
{
    pub fn new(
        kind: StarSub,
        tx: mpsc::Sender<StarCall<P>>,
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
        self.tx.send(StarCall::Init).await;
    }

    pub async fn from_hyperway(&self, wave: UltraWave, results: bool) -> Result<(), MsgErr> {
        match results {
            true => {
                let (tx, mut rx) = oneshot::channel();
                self.tx
                    .send(StarCall::FromHyperway {
                        wave,
                        rtn: Some(tx),
                    })
                    .await;
                rx.await?
            }
            false => {
                self.tx
                    .send(StarCall::FromHyperway { wave, rtn: None })
                    .await;
                Ok(())
            }
        }
    }

    pub async fn traverse_to_next_layer(&self, traversal: Traversal<UltraWave>) {
        self.tx.send(StarCall::TraverseToNextLayer(traversal)).await;
    }

    pub async fn inject_traversal(&self, inject: TraversalInjection) {
        self.tx
            .send(StarCall::LayerTraversalInjection(inject))
            .await;
    }

    pub async fn stub(&self) -> Result<StarStub, MsgErr> {
        let (tx, rx) = oneshot::channel();
        self.tx.send(StarCall::Stub(tx)).await;
        Ok(rx.await?)
    }

    pub async fn create_states(&self, point: Point) -> Result<(), MsgErr> {
        let (rtn, rtn_rx) = oneshot::channel();
        self.tx.send(StarCall::CreateStates { point, rtn }).await;
        rtn_rx.await?;
        Ok(())
    }

    #[cfg(test)]
    pub async fn get_skel(&self) -> Result<StarSkel<P>, MsgErr> {
        let (tx, rx) = oneshot::channel();
        self.tx.send(StarCall::GetSkel(tx)).await;
        Ok(rx.await?)
    }

    #[cfg(test)]
    pub async fn to_gravity(&self, wave: UltraWave) {
        self.tx.send(StarCall::ToGravity(wave)).await;
    }
}

pub struct Star<P>
where
    P: Platform + 'static,
{
    skel: StarSkel<P>,
    star_tx: mpsc::Sender<StarCall<P>>,
    star_rx: mpsc::Receiver<StarCall<P>>,
    drivers: DriversApi<P>,
    injector: Port,
    forwarders: Vec<Point>,
    golden_path: DashMap<StarKey, StarKey>,
    hyperway_transmitter: ProtoTransmitter,
    gravity: Port,
    hyper_router: Arc<dyn Router>,
    layer_traversal_engine: LayerTraversalEngine<P>,
}

impl<P> Star<P>
where
    P: Platform,
{
    pub async fn new(
        skel: StarSkel<P>,
        mut drivers: DriversBuilder<P>,
        mut hyperway_ext: HyperwayExt,
        mut star_tx: StarTx<P>,
    ) -> Result<StarApi<P>, P::Err> {
        let drivers = drivers.build(
            skel.clone(),
            star_tx.drivers_call_tx.clone(),
            star_tx.drivers_call_rx.take().unwrap(),
            star_tx.drivers_status_tx.take().unwrap(),
            star_tx.drivers_status_rx.clone(),
        );

        let star_rx = star_tx.call_rx.take().unwrap();
        let star_tx = star_tx.call_tx;

        let mut forwarders = vec![];
        for (point, stub) in skel.adjacents.iter() {
            if stub.kind.is_forwarder() {
                forwarders.push(point.clone());
            }
        }

        let hyper_router = Arc::new(hyperway_ext.router());
        let mut hyperway_transmitter =
            ProtoTransmitterBuilder::new(hyper_router.clone(), skel.exchanger.clone());
        hyperway_transmitter.agent = SetStrategy::Override(Agent::HyperUser);
        hyperway_transmitter.scope = SetStrategy::Override(Scope::Full);
        let hyperway_transmitter = hyperway_transmitter.build();

        let mut injector = skel
            .location()
            .clone()
            .push("injector")
            .unwrap()
            .to_port()
            .with_layer(Layer::Gravity);

        let (to_gravity_traversal_tx, mut to_gravity_traversal_rx): (
            mpsc::Sender<Traversal<UltraWave>>,
            mpsc::Receiver<Traversal<UltraWave>>,
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

        let mut golden_path = DashMap::new();
        for con in skel.connections.iter() {
            golden_path.insert(con.key().clone(), con.key().clone());
        }

        let gravity = skel.point.clone().to_port().with_layer(Layer::Gravity);

        // relay from hyperway_ext
        {
            let star_tx = star_tx.clone();
            tokio::spawn(async move {
                while let Some(wave) = hyperway_ext.rx.recv().await {
                    star_tx
                        .send(StarCall::FromHyperway { wave, rtn: None })
                        .await;
                }
            });
        }

        {
            let mut drivers = drivers.clone();
            let status_tx = skel.status_tx.clone();
            tokio::spawn(async move {
                loop {
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
                            status_tx.send(Status::Ready).await;
                        }
                        DriverStatus::Retrying(_) => {
                            status_tx.send(Status::Panic).await;
                        }
                        DriverStatus::Fatal(_) => {
                            status_tx.send(Status::Fatal).await;
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

        let kind = skel.kind.clone();
        {
            let star = Self {
                skel,
                star_tx: star_tx.clone(),
                star_rx,
                drivers,
                injector,
                golden_path,
                hyperway_transmitter,
                forwarders,
                gravity,
                hyper_router,
                layer_traversal_engine,
            };
            star.start();
        }

        Ok(StarApi::new(kind, star_tx, status_rx))
    }

    fn start(mut self) {
        tokio::spawn(async move {
            while let Some(call) = self.star_rx.recv().await {
                match call {
                    StarCall::Init => {
                        self.drivers.init().await;
                    }
                    StarCall::FromHyperway { wave, rtn } => {
                        let result = self
                            .from_hyperway(wave)
                            .await
                            .map_err(|e| e.to_cosmic_err());
                        if let Some(tx) = rtn {
                            tx.send(result);
                        } else {
                            match result {
                                Ok(_) => {}
                                Err(e) => {
                                    self.skel.err(e.to_string());
                                }
                            }
                        }
                    }
                    StarCall::TraverseToNextLayer(traversal) => {
                        let layer_traversal_engine = self.layer_traversal_engine.clone();
                        tokio::spawn(async move {
                            layer_traversal_engine.traverse_to_next_layer(traversal).await;
                        });
                    }
                    StarCall::LayerTraversalInjection(inject) => {
                        let layer_traversal_engine = self.layer_traversal_engine.clone();
                        tokio::spawn(async move {
                            layer_traversal_engine
                                .start_layer_traversal(inject.wave, &inject.injector, false)
                                .await;
                        });
                    }
                    StarCall::Stub(rtn) => {
                        rtn.send(self.skel.stub());
                    }
                    StarCall::Phantom(_) => {
                        // phantom literally does nothing but hold the P in not test mode
                    }
                    StarCall::ToDriver(traversal) => {
                        self.drivers.visit(traversal).await;
                    }
                    StarCall::ToGravity(wave) => match self.to_gravity(wave).await {
                        Ok(_) => {}
                        Err(err) => {
                            self.skel.err(err.to_string());
                        }
                    },
                    #[cfg(test)]
                    StarCall::GetSkel(rtn) => {
                        rtn.send(self.skel.clone());
                    }
                    StarCall::CreateStates { point, rtn } => {
                        self.create_states(point).await;
                        rtn.send(());
                    }
                    StarCall::Shard(wave) => {
                        self.shard(wave).await;
                    }
                    StarCall::Assign { details, rtn } => {
                        rtn.send(self.assign(details).await);
                    }
                }
            }
        });
    }

    async fn create_states(&self, point: Point) {
        self.skel.state.create_field(point.clone());
        self.skel.state.create_shell(point.clone());
    }

    async fn init_drivers(&self) {
        self.skel.logger.info("Star::init_drivers()");
        self.drivers.init().await;
    }

    // receive a wave from the hyperlane... this wave should always be
    // a Wave<Signal> of the SysMethod<Hop> which should in turn contain a SysMethod<Transport> Signal
    #[track_caller]
    async fn from_hyperway(&self, wave: UltraWave) -> Result<(), P::Err> {
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
            return self.skel.err("transport signal exceeded max hops");
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
                layer_engine
                    .start_layer_traversal(transport.to_ultra(), &injector, true)
                    .await
            });
            Ok(())
        } else {
            self.forward(transport).await
        }
    }

    // send this transport signal towards it's destination
    async fn forward(&self, transport: Wave<Signal>) -> Result<(), P::Err> {
        if self.skel.kind.is_forwarder() {
            self.to_hyperway(transport).await
        } else {
            self.skel.err(format!(
                "attempt to forward a transport on a non forwarding Star Kind: {}",
                self.skel.kind.to_string()
            ))
        }
    }
    // sending a wave that is from and to a particle into the fabric...
    // here it will be wrapped into a transport for star to star delivery or
    // sent to GLOBAL::registry if addressed in such a way
    #[track_caller]
    async fn to_gravity(&self, wave: UltraWave) -> Result<(), P::Err> {
        #[cfg(test)]
        self.skel
            .diagnostic_interceptors
            .to_gravity
            .send(wave.clone())
            .unwrap_or_default();
        let logger = self.skel.logger.push_mark("hyperstar:to-gravity").unwrap();
        logger.track(&wave, || Tracker::new("to_gravity", "Receive"));
        if wave.is_directed()
            && wave.to().is_single()
            && wave.to().to_single().unwrap().point == *GLOBAL_EXEC
        {
            let mut wave = wave;
            wave.set_to(self.skel.machine.global.clone());
            let mut transport = wave
                .wrap_in_transport(self.gravity.clone(), self.skel.machine.machine_star.clone());
            let transport = transport.build()?;
            let transport = transport.to_signal()?;
            self.to_hyperway(transport).await?;
        } else {
                logger
                .result(self.star_tx.send(StarCall::Shard(wave)).await)
                .unwrap_or_default();

            // move to Shard
            /*            let waves =
                           self.shard(wave, &self.skel.adjacents, &self.skel.registry)
                               .await?;
                       for (to, wave) in waves {
                           let mut transport = wave.wrap_in_transport(self.gravity.clone(), to.to_port());
                           transport.from(self.skel.point.clone().to_port());
                           let transport = transport.build()?;
                           let transport = transport.to_signal()?;
                           self.to_hyperway(transport).await?;
                       }

            */
        }
        Ok(())
    }

    #[track_caller]
    async fn shard(&self, wave: UltraWave) -> Result<(), P::Err> {
        let logger = self.skel.logger.push_mark("hyperstar:shard").unwrap();
        match wave {
            _ => {
                let to = wave.to().unwrap_single();
                let point = &to.point;
                let record = self.skel.registry.locate(point).await?;

                if record.location.is_none() {
                    let logger = logger.push_mark("provision").unwrap();
                    let (rtn, mut rtn_rx) = oneshot::channel();
                    let details = record.details;
                        logger.result(self.star_tx.send(StarCall::Assign { details, rtn }).await).unwrap_or_default();
                    let star_tx = self.star_tx.clone();
                    let registry = self.skel.registry.clone();
                    let timeouts = self.skel.exchanger.timeouts.clone();
                    let assign = registry.assign(&to.point);
                    tokio::spawn(async move {
                        async fn get(
                            rtn_rx: oneshot::Receiver<Result<Point, MsgErr>>,
                            assign: oneshot::Sender<Point>,
                            timeouts: Timeouts,
                        ) -> Result<(), MsgErr> {
                            let point = tokio::time::timeout(
                                Duration::from_secs(timeouts.from(WaitTime::High)),
                                rtn_rx,
                            )
                            .await???;
                            assign.send(point).unwrap();
                            Ok(())
                        }

                        logger
                            .result(get(rtn_rx, assign, timeouts).await)
                            .unwrap_or_default();
                        logger
                            .result(star_tx.send(StarCall::Shard(wave)).await)
                            .unwrap_or_default();
                    });
                } else {
                    let logger = logger.push_mark("transport").unwrap();
                    let mut transport = wave.wrap_in_transport(
                        self.gravity.clone(),
                        record.location.unwrap().to_port(),
                    );
                    transport.from(self.skel.point.clone().to_port());
                    let transport = transport.build()?;
                    let transport = transport.to_signal()?;
                        logger .result(self.to_hyperway(transport).await) .unwrap_or_default();
                }
            } /*
              UltraWave::Ripple(ripple) => {
                  unimplemented!();
                              let mut map = shard_ripple_by_location(ripple, adjacent, registry).await?;
                              Ok(map
                                  .into_iter()
                                  .map(|(point, ripple)| (point, ripple.to_ultra()))
                                  .collect())

              }
               */
        }
        Ok(())
    }

    #[track_caller]
    async fn assign(&self, details: Details) -> Result<Point, MsgErr> {
        let logger = self.skel.logger.push_mark("assign")?;
        let assign = Assign::new(AssignmentKind::Create, details.clone(), StateSrc::None);

        let mut wave = DirectedProto::ping();
        wave.method(SysMethod::Assign);
        wave.body(Sys::Assign(assign).into());
        wave.from(self.skel.point.clone().to_port().with_layer(Layer::Core));
        wave.to(details
            .stub
            .point
            .parent()
            .ok_or("expected parent for assignment")?
            .to_port()
            .with_layer(Layer::Gravity));

        let pong: Wave<Pong> = logger.result(self.skel.gravity_transmitter.direct(wave).await)?;

        if pong.core.status.as_u16() == 200 {
            if let Substance::Point(location) = &pong.core.body {
                Ok(location.clone())
            } else {
                logger.result(Err(
                    P::Err::new("Assign result expected Substance Point").to_cosmic_err()
                ))
            }
        } else {
            let logger = logger.push_mark("assign-failure")?;
                logger
                .result(
                    self.skel
                        .registry
                        .set_status(&details.stub.point, &Status::Panic)
                        .await,
                )
                .unwrap_or_default();
            logger.result(Err(P::Err::new(format!(
                "not success on assign {}",
                details.stub.point.to_string()
            ))
            .to_cosmic_err()))
        }
    }

    // send this transport signal into the hyperway
    // wrap the transport into a hop to go to one and only one star

    #[track_caller]
    async fn to_hyperway(&self, transport: Wave<Signal>) -> Result<(), P::Err> {
        let logger = self.skel.logger.push_mark("hyperstar:to-hyperway")?;
        if self.skel.point == transport.to.point {
            // it's a bit of a strange case, but even if this star is sending a transport message
            // to itself, it still makes use of the Hyperway Interchange, which will bounce it back
            // The reason for this is that it is the Hyperway that handles things like Priority, Urgency
            // and hopefully in the future durability, whereas within the star itself all waves are
            // treated equally.
            logger.result(self.hyperway_transmitter
                .direct(
                    transport.wrap_in_hop(self.gravity.clone(), self.skel.point.clone().to_port()),
                )
                .await)?;
            Ok(())
        } else if self.skel.adjacents.contains_key(&transport.to.point) {
            let to = transport.to.clone();
            logger.result(self.hyperway_transmitter
                .direct(transport.wrap_in_hop(self.gravity.clone(), to))
                .await)?;
            Ok(())
        } else if self.forwarders.len() == 1 {
            let to = self.forwarders.first().unwrap().clone().to_port();
            logger.result(self.hyperway_transmitter
                .direct(transport.wrap_in_hop(self.gravity.clone(), to))
                .await)?;
            Ok(())
        } else if self.forwarders.is_empty() {
            self.skel.err("this star needs to send a transport to a non-adjacent star yet does not have any adjacent forwarders")
        } else {
            unimplemented!("need to now send out a ripple search for the star being transported to")
        }
    }

    #[track_caller]
    async fn find_next_hop(&self, star_key: &StarKey) -> Result<Option<StarKey>, MsgErr> {
        let logger = self.skel.logger.push_mark("hyperstar:find_next_hop")?;
        if let Some(adjacent) = self.golden_path.get(star_key) {
            Ok(Some(adjacent.value().clone()))
        } else {
println!("Find next hop...");
            let mut ripple = DirectedProto::ping();
            ripple.kind(DirectedKind::Ripple);
            ripple.method(SysMethod::Search);
            ripple.body(Substance::Sys(Sys::Search(Search::Star(star_key.clone()))));
            ripple.bounce_backs = Some(BounceBacks::Count(self.skel.adjacents.len()));
            ripple.to(Recipients::Stars);
            let echoes: Echoes = self.skel.gravity_transmitter.direct(ripple).await?;

            let mut coalated = vec![];
            for echo in echoes {
                if let Substance::Sys(Sys::Discoveries(discoveries)) = &echo.core.body {
                    for discovery in discoveries.iter() {
                        coalated.push(StarDiscovery::new(
                            StarPair::new(
                                self.skel.key.clone(),
                                StarKey::try_from(echo.from.point.clone())?,
                            ),
                            discovery.clone(),
                        ));
                    }
                } else {
                        logger
                        .warn("unexpected reflected core substance from search echo");
                }
            }

            coalated.sort();

            match coalated.first() {
                None => Ok(None),
                Some(discovery) => {
                    let key = discovery.pair.not(&self.skel.key).clone();
                    self.golden_path.insert(star_key.clone(), key.clone());
                    Ok(Some(key))
                }
            }
        }
    }

    /*
    async fn re_ripple( &self, ripple: Wave<Ripple> ) -> Result<Vec<Echoes>,MsgErr> {
        let mut reflections: Vec<BoxFuture<Echoes>> = vec![];

        for (location, ripple) in ripple.shard_by_location(&self.skel.adjacents, &self.skel.registry ).await?
        {
           if !ripple.history.contains(&location) {
               let key = StarKey::try_from(location)?;
               let adjacent = self.find_next_hop(&key).await?.ok_or("could not find golden way")?.to_port();
               let mut wave = DirectedProto::new();
               wave.kind(DirectedKind::Signal);
               wave.to(adjacent);
               wave.method(SysMethod::Transport.into());
               wave.handling(ripple.handling.clone());
               wave.agent(Agent::HyperUser);
               wave.body(Substance::UltraWave(Box::new(ripple.to_ultra())));
               reflections.push(self.skel.gravity_well_transmitter.direct(wave).boxed());
           }
        }

        if reflections.is_empty() {
            return Ok(vec![]);
        }

        let echoes = join_all(reflections).await?;

        Ok(echoes)
    }
     */
}

#[derive(Clone)]
pub struct LayerTraversalEngine<P>
where
    P: Platform + 'static,
{
    pub skel: StarSkel<P>,
    pub injector: Port,
    pub exit_up: mpsc::Sender<Traversal<UltraWave>>,
    pub exit_down: mpsc::Sender<Traversal<UltraWave>>,
    pub layers: HashSet<Layer>,
}

impl<P> LayerTraversalEngine<P>
where
    P: Platform + 'static,
{
    pub fn new(
        skel: StarSkel<P>,
        injector: Port,
        exit_down: mpsc::Sender<Traversal<UltraWave>>,
        exit_up: mpsc::Sender<Traversal<UltraWave>>,
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

    async fn start_layer_traversal(
        &self,
        mut wave: UltraWave,
        injector: &Port,
        from_hyperway: bool,
    ) -> Result<(), P::Err> {
        #[cfg(test)]
        self.skel
            .diagnostic_interceptors
            .start_layer_traversal_wave
            .send(wave.clone())
            .unwrap_or_default();


        let logger = self.skel.logger.push_mark("hyperstar:start-layer-traversal")?;

        let mut to = wave.to().clone().unwrap_single();
        if to.point == *GLOBAL_EXEC {
panic!("GLOBAL EXEC");
            wave.set_to(self.skel.machine.global.clone());
            to = self.skel.machine.global.clone();
        }

        let record = match self.skel.registry.locate(&to.point).await {
            Ok(record) => record,
            Err(err) => {
                return self.skel.err(format!(
                    "could not locate record for surface {} from {}",
                    to.to_string(), wave.from().to_string()
                ));
            }
        };

        let plan = record.details.stub.kind.wave_traversal_plan().clone();

        let mut dest = None;
        let mut dir = TraversalDirection::Core;
        // now we check if we are doing an inter point delivery (from one layer to another in the same Particle)
        // if this delivery was from_hyperway, then it was certainly a message being routed back to the star
        // and is not considered an inter point delivery
        if !from_hyperway && wave.to().clone().unwrap_single().point == wave.from().point {
            // it's the SAME point, so the to layer becomes our dest
            dest.replace(wave.to().clone().unwrap_single().layer);

            // make sure we have this layer in the plan
            if !plan.has_layer(&wave.to().clone().unwrap_single().layer) {
                return self.skel.err("attempt to send wave to layer that the recipient Kind does not have in its traversal plan");
            }

            // dir is from inject_layer to dest
            dir = match TraversalDirection::new(
                &injector.layer,
                &wave.to().clone().unwrap_single().layer,
            ) {
                Ok(dir) => dir,
                Err(_) => {
                    // looks like we are already on the dest layer...
                    // that means it doesn't matter what the TraversalDirection is
                    TraversalDirection::Fabric
                }
            }
        } else {
            // if this wave was injected by the from Particle, then we need to first
            // traverse towards the fabric
            if injector.point == wave.from().point {
                dir = TraversalDirection::Fabric;
            } else {
                // if this was injected by something else (like the Star)
                // then it needs to traverse towards the Core
                dir = TraversalDirection::Core;
                // and dest will be the to layer
                if !from_hyperway {
                    dest.replace(wave.to().clone().unwrap_single().layer);
                }
            }
        }

        let traversal_logger = self
            .skel
            .logger
            .point(wave.to().clone().unwrap_single().to_point());
        let traversal_logger = traversal_logger.span();
        let to = wave.to().clone().unwrap_single();

        let point = if *injector == self.injector {
            // if injected by the star then the destination is the point that this traversal belongs to
            to.clone().to_point()
        } else {
            // if injected by any other point then the injector is the point that this traversal belongs to
            injector.clone().to_point()
        };

        let mut traversal = Traversal::new(
            wave,
            record,
            injector.layer.clone(),
            traversal_logger,
            dir,
            dest,
            to,
            point,
        );

        // in the case that we injected into a layer that is not part
        // of this plan, we need to send the traversal to the next layer
        if !plan.has_layer(&injector.layer) {
            match traversal.next() {
                None => {
                    self.exit(traversal).await;
                    return Ok(());
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
        match self.visit_layer(traversal).await {
            Ok(_) => Ok(()),
            Err(err) => {
                logger.error(format!("{}", err.to_string()));
                Err(err.into())
            }
        }
    }

    async fn exit(&self, traversal: Traversal<UltraWave>) -> Result<(), MsgErr> {
        match traversal.dir {
            TraversalDirection::Fabric => {
                self.exit_up.send(traversal).await;
                return Ok(());
            }
            TraversalDirection::Core => {
                self.exit_down.send(traversal).await;
                return Ok(());
            }
        }
    }

    async fn visit_layer(&self, traversal: Traversal<UltraWave>) -> Result<(), MsgErr> {
        let logger = self.skel.logger.push_mark("stack-traversal:visit")?;
        logger.track(&traversal, || {
            Tracker::new(
                format!("visit:layer@{}", traversal.layer.to_string()),
                "Visit",
            )
        });

        match traversal.layer {
            Layer::Field => {
                let field = Field::new(
                    traversal.point.clone(),
                    self.skel.clone(),
                    self.skel
                        .state
                        .find_field(&traversal.to.clone().with_layer(Layer::Field))?,
                    traversal.logger.clone(),
                );
                tokio::spawn(async move {
                    let logger = logger.push_action("Field").unwrap();
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
                        .find_shell(&traversal.to.clone().with_layer(Layer::Shell))?,
                );

                let logger = logger.clone();
                tokio::spawn(async move {
                    let logger = logger.push_action("Shell").unwrap();
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

    async fn traverse_to_next_layer(&self, mut traversal: Traversal<UltraWave>) {
        let logger = self.skel.logger.push_mark("stack-traversal:traverse-to-next-layer").unwrap();
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
                        logger
                        .warn("should not have traversed a wave all the way to the core in Star");
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
    pub tx: mpsc::Sender<UltraWave>,
}

#[derive(Clone)]
pub struct LayerInjectionRouter<P>
where
    P: Platform + 'static,
{
    pub skel: StarSkel<P>,
    pub injector: Port,
}

impl<P> LayerInjectionRouter<P>
where
    P: Platform + 'static,
{
    pub fn new(skel: StarSkel<P>, injector: Port) -> Self {
        Self { skel, injector }
    }

    pub fn with(&self, injector: Port) -> Self {
        Self {
            skel: self.skel.clone(),
            injector,
        }
    }
}

#[async_trait]
impl<P> Router for LayerInjectionRouter<P>
where
    P: Platform,
{
    async fn route(&self, wave: UltraWave) {
        let inject = TraversalInjection::new(self.injector.clone(), wave);
        self.skel.inject_tx.send(inject).await;
    }

    fn route_sync(&self, wave: UltraWave) {
        let inject = TraversalInjection::new(self.injector.clone(), wave);
        self.skel.inject_tx.try_send(inject);
    }
}

pub trait TopicHandler: Send + Sync + DirectedHandler {
    fn source_selector(&self) -> &PortSelector;
}

pub trait TopicHandlerSerde<T: TopicHandler> {
    fn serialize(&self, handler: T) -> Substance;
    fn deserialize(&self, ser: Substance) -> T;
}

impl StateApi {
    pub fn new(tx: mpsc::Sender<StateCall>) -> Self {
        Self {
            tx,
            layer_filter: None,
        }
    }

    pub fn with_layer(self, layer: Layer) -> Self {
        Self {
            tx: self.tx,
            layer_filter: Some(layer),
        }
    }
}

#[derive(Clone)]
pub struct StateApi {
    pub tx: mpsc::Sender<StateCall>,
    layer_filter: Option<Layer>,
}

impl StateApi {
    pub async fn get_state(&self, port: Port) -> Result<Option<Arc<RwLock<dyn State>>>, MsgErr> {
        if let Some(layer) = &self.layer_filter {
            if port.layer != *layer {
                return Err(MsgErr::forbidden_msg(format!(
                    "not allowed to get state from Port Layer {} try layer {}",
                    port.layer.to_string(),
                    layer.to_string()
                )));
            }
        }
        let (tx, rx) = oneshot::channel();
        self.tx.send(StateCall::Get { port, tx }).await;
        rx.await?
    }

    pub async fn put_state(&self, port: Port, state: Arc<RwLock<dyn State>>) -> Result<(), MsgErr> {
        if let Some(layer) = &self.layer_filter {
            if port.layer != *layer {
                return Err(MsgErr::forbidden_msg(format!(
                    "not allowed to put state on Port Layer {} try layer {}",
                    port.layer.to_string(),
                    layer.to_string()
                )));
            }
        }
        let (tx, rx) = oneshot::channel();
        self.tx.send(StateCall::Put { port, state, tx }).await;
        rx.await?
    }
}

pub enum StateCall {
    Get {
        port: Port,
        tx: oneshot::Sender<Result<Option<Arc<RwLock<dyn State>>>, MsgErr>>,
    },
    Put {
        port: Port,
        state: Arc<RwLock<dyn State>>,
        tx: oneshot::Sender<Result<(), MsgErr>>,
    },
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

#[derive(Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct StarPair {
    pub a: StarKey,
    pub b: StarKey,
}

impl StarPair {
    pub fn new(a: StarKey, b: StarKey) -> Self {
        if a < b {
            Self { a, b }
        } else {
            Self { a: b, b: a }
        }
    }

    pub fn not(&self, this: &StarKey) -> &StarKey {
        if self.a == *this {
            &self.b
        } else {
            &self.a
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct StarDiscovery {
    pub pair: StarPair,
    pub discovery: Discovery,
}

impl Deref for StarDiscovery {
    type Target = Discovery;

    fn deref(&self) -> &Self::Target {
        &self.discovery
    }
}

impl StarDiscovery {
    pub fn new(pair: StarPair, discovery: Discovery) -> Self {
        Self { pair, discovery }
    }
}

impl Ord for StarDiscovery {
    fn cmp(&self, other: &Self) -> Ordering {
        if self.discovery.hops != other.discovery.hops {
            self.discovery.hops.cmp(&other.discovery.hops)
        } else {
            self.pair.cmp(&other.pair)
        }
    }
}

impl PartialOrd for StarDiscovery {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self.discovery.hops != other.discovery.hops {
            self.discovery.hops.partial_cmp(&other.discovery.hops)
        } else {
            self.pair.partial_cmp(&other.pair)
        }
    }
}

lazy_static! {
    static ref STAR_BIND_CONFIG: ArtRef<BindConfig> = ArtRef::new(
        Arc::new(star_bind()),
        Point::from_str("GLOBAL::repo:1.0.0:/bind/star.bind").unwrap()
    );
}

fn star_bind() -> BindConfig {
    log(bind_config(
        r#"
    Bind(version=1.0.0)
    {
       Route<Sys<Transport>> -> (());
       Route<Sys<Assign>> -> (()) => &;
    }
    "#,
    ))
    .unwrap()
}

#[derive(Clone)]
pub struct StarDriverFactory<P>
where
    P: Platform + 'static,
{
    pub kind: Kind,
    pub phantom: PhantomData<P>,
}

impl<P> StarDriverFactory<P>
where
    P: Platform + 'static,
{
    pub fn new(kind: StarSub) -> Self {
        Self {
            kind: Kind::Star(kind),
            phantom: Default::default(),
        }
    }
}

#[async_trait]
impl<P> HyperDriverFactory<P> for StarDriverFactory<P>
where
    P: Platform + 'static,
{
    fn kind(&self) -> Kind {
        self.kind.clone()
    }

    async fn create(
        &self,
        star: StarSkel<P>,
        skel: DriverSkel<P>,
        ctx: DriverCtx,
    ) -> Result<Box<dyn Driver<P>>, P::Err> {
        Ok(Box::new(StarDriver::new(star, skel)))
    }
}

#[derive(DirectedHandler)]
pub struct StarDriver<P>
where
    P: Platform + 'static,
{
    pub star_skel: StarSkel<P>,
    pub driver_skel: DriverSkel<P>,
}

impl<P> StarDriver<P>
where
    P: Platform,
{
    pub fn new(star_skel: StarSkel<P>, driver_skel: DriverSkel<P>) -> Self {
        Self {
            star_skel,
            driver_skel,
        }
    }

    async fn search_for_stars(&self, search: Search) -> Result<Vec<Discovery>, MsgErr> {
        let mut ripple = DirectedProto::ping();
        ripple.kind(DirectedKind::Ripple);
        ripple.method(SysMethod::Search);
        ripple.bounce_backs = Some(BounceBacks::Count(self.star_skel.adjacents.len()));
        ripple.body(Substance::Sys(Sys::Search(search)));
        ripple.to(Recipients::Stars);
        let echoes: Echoes = self.star_skel.gravity_transmitter.direct(ripple).await?;

        let mut rtn = vec![];
        for echo in echoes {
            if let Substance::Sys(Sys::Discoveries(discoveries)) = echo.variant.core.body {
                for discovery in discoveries.vec.into_iter() {
                    rtn.push(discovery);
                }
            } else {
                self.star_skel
                    .logger
                    .warn("unexpected reflected core substance from search echo");
            }
        }

        Ok(rtn)
    }
}

#[async_trait]
impl<P> Driver<P> for StarDriver<P>
where
    P: Platform,
{
    fn kind(&self) -> Kind {
        Kind::Star(self.star_skel.kind.clone())
    }

    async fn init(&mut self, skel: DriverSkel<P>, ctx: DriverCtx) -> Result<(), P::Err> {
        let logger = skel.logger.push_mark("init")?;
            logger
            .result(self.driver_skel.status_tx.send(DriverStatus::Init).await)
            .unwrap_or_default();

        let point = self.star_skel.point.clone();
        let registration = Registration {
            point: point.clone(),
            kind: Kind::Star(self.star_skel.kind.clone()),
            registry: Default::default(),
            properties: Default::default(),
            owner: HYPERUSER.clone(),
            strategy: Strategy::Override,
            status: Status::Ready,
        };

        self.star_skel.api.create_states(point.clone()).await?;
        self.star_skel.registry.register(&registration).await?;
        self.star_skel
            .registry
            .assign(&point)
            .send(self.star_skel.point.clone());

            logger
            .result(skel.status_tx.send(DriverStatus::Ready).await)
            .unwrap_or_default();

        Ok(())
    }

    async fn item(&self, point: &Point) -> Result<ItemHandler<P>, P::Err> {
        Ok(ItemHandler::Handler(Box::new(StarCore::restore(
            self.star_skel.clone(),
            (),
            (),
        ))))
    }

    async fn assign(&self, assign: Assign) -> Result<(), P::Err> {
        Err("only allowed one Star per StarDriver".into())
    }
}

#[routes]
impl<P> StarDriver<P> where P: Platform {}

#[derive(DirectedHandler)]
pub struct StarCore<P>
where
    P: Platform + 'static,
{
    pub skel: StarSkel<P>,
}

#[async_trait]
impl<P> ItemDirectedHandler<P> for StarCore<P>
where
    P: Platform,
{
    async fn bind(&self) -> Result<ArtRef<BindConfig>, P::Err> {
        <StarCore<P> as Item<P>>::bind(self).await
    }
}

#[async_trait]
impl<P> Item<P> for StarCore<P>
where
    P: Platform + 'static,
{
    type Skel = StarSkel<P>;
    type Ctx = ();
    type State = ();

    fn restore(skel: Self::Skel, ctx: Self::Ctx, state: Self::State) -> Self {
        StarCore { skel }
    }

    async fn bind(&self) -> Result<ArtRef<BindConfig>, P::Err> {
        Ok(STAR_BIND_CONFIG.clone())
    }
}

#[routes]
impl<P> StarCore<P>
where
    P: Platform,
{
    #[track_caller]
    #[route("Sys<Assign>")]
    pub async fn assign(&self, ctx: InCtx<'_, Sys>) -> Result<ReflectedCore, P::Err> {
        self.skel.logger.info("***> ASSIGN!!!");
        if let Sys::Assign(assign) = ctx.input {
            #[cfg(test)]
            self.skel
                .diagnostic_interceptors
                .assignment
                .send(assign.clone())
                .unwrap_or_default();

            if self
                .skel
                .drivers
                .kinds()
                .await?
                .contains(&assign.details.stub.kind)
            {
                // create field and shell
                self.skel
                    .state
                    .create_field(assign.details.stub.point.clone());
                self.skel
                    .state
                    .create_shell(assign.details.stub.point.clone());

                self.skel
                    .logger
                    .result(self.skel.drivers.assign(assign.clone()).await)?;
            } else {
                error!(
                    "do not have a driver for kind: {}",
                    assign.details.stub.kind.to_string()
                );
            }

            Ok(ReflectedCore::ok())
        } else {
            Err("expected Sys<Assign>".into())
        }
    }

    #[route("Sys<Transport>")]
    pub async fn transport(&self, ctx: InCtx<'_, UltraWave>) {
        #[cfg(test)]
        self.skel
            .diagnostic_interceptors
            .transport_endpoint
            .send(ctx.wave().clone().to_ultra())
            .unwrap_or_default();

        self.skel.logger.track(ctx.wave(), || {
            Tracker::new("star:core:transport", "Receive")
        });

        let wave = ctx.input.clone();

        let injection = TraversalInjection::new(
            self.skel.point.clone().to_port().with_layer(Layer::Gravity),
            wave,
        );

        self.skel.inject_tx.send(injection).await;
    }

    #[route("Sys<Search>")]
    pub async fn handle_search_request(&self, ctx: InCtx<'_, Sys>) -> CoreBounce {
        fn reflect<'a, E>(core: &StarCore<E>, ctx: &'a InCtx<'a, Sys>) -> ReflectedCore
        where
            E: Platform,
        {
            unimplemented!()
            /*
            let discovery = Discovery {
                star_kind: core.skel.kind.clone(),
                hops: ctx.wave().hops(),
                star_key: core.skel.key.clone(),
                kinds: core.skel.kinds.clone(),
            };
            let mut core = ReflectedCore::new();
            let mut discoveries = Discoveries::new();
            discoveries.push(discovery);
            core.body = Substance::Sys(Sys::Discoveries(discoveries));
            core.status = StatusCode::from_u16(200).unwrap();
            core
             */
        }

        if let Sys::Search(search) = ctx.input {
            match search {
                Search::Star(star) => {
                    if self.skel.key == *star {
                        return CoreBounce::Reflected(reflect(self, &ctx));
                    }
                }
                Search::StarKind(kind) => {
                    if *kind == self.skel.kind {
                        return CoreBounce::Reflected(reflect(self, &ctx));
                    }
                }
                Search::Kinds => {
                    return CoreBounce::Reflected(reflect(self, &ctx));
                }
            }
            return CoreBounce::Absorbed;
        } else {
            return CoreBounce::Reflected(ctx.bad_request());
        }
    }
}

#[derive(Clone)]
pub struct StarWrangles {
    pub wrangles: Arc<DashMap<Kind, RoundRobinWrangleSelector>>,
}

impl StarWrangles {
    pub fn new() -> Self {
        Self {
            wrangles: Arc::new(DashMap::new()),
        }
    }

    pub fn add(&self, discoveries: Vec<StarDiscovery>) {
        for discovery in discoveries {
            for kind in discovery.kinds.clone() {
                match self.wrangles.get_mut(&kind) {
                    None => {
                        let mut wrangler = RoundRobinWrangleSelector::new(kind.clone());
                        wrangler.stars.push(discovery.clone());
                        wrangler.sort();
                    }
                    Some(mut wrangler) => {
                        let mut wrangler = wrangler.value_mut();
                        wrangler.stars.push(discovery.clone());
                        wrangler.sort();
                    }
                }
            }
        }
    }

    pub fn verify(&self, kinds: &[&Kind]) -> Result<(), MsgErr> {
        for kind in kinds {
            if !self.wrangles.contains_key(*kind) {
                return Err(format!(
                    "star must be able to wrangle at least one {}",
                    kind.to_string()
                )
                .into());
            }
        }
        Ok(())
    }

    pub async fn wrangle(&self, kind: &Kind) -> Result<StarKey, MsgErr> {
        self.wrangles
            .get(kind)
            .ok_or(format!(
                "could not find wrangles for kind {}",
                kind.to_string()
            ))?
            .value()
            .wrangle()
            .await
    }
}

pub struct RoundRobinWrangleSelector {
    pub kind: Kind,
    pub stars: Vec<StarDiscovery>,
    pub index: Mutex<usize>,
    pub step_index: usize,
}

impl RoundRobinWrangleSelector {
    pub fn new(kind: Kind) -> Self {
        Self {
            kind,
            stars: vec![],
            index: Mutex::new(0),
            step_index: 0,
        }
    }

    pub fn sort(&mut self) {
        self.stars.sort();
        self.step_index = 0;
        let mut hops: i32 = -1;
        for discovery in &self.stars {
            if hops < 0 {
                hops = discovery.hops as i32;
            } else if discovery.hops as i32 > hops {
                break;
            }
            self.step_index += 1;
        }
    }

    pub async fn wrangle(&self) -> Result<StarKey, MsgErr> {
        if self.stars.is_empty() {
            return Err(format!("cannot find wrangle for kind: {}", self.kind.to_string()).into());
        }

        let index = {
            let mut lock = self.index.lock().await;
            let index = *lock;
            lock.add(1);
            index
        };

        let index = index % self.step_index;

        if let Some(discovery) = self.stars.get(index) {
            Ok(discovery.discovery.star_key.clone())
        } else {
            Err(format!("cannot find wrangle for kind: {}", self.kind.to_string()).into())
        }
    }
}

/*
pub async fn shard_ripple_by_location<E>(
    ripple: Wave<Ripple>,
    adjacent: &HashMap<Point, StarStub>,
    registry: &Registry<E>,
) -> Result<HashMap<Point, Wave<Ripple>>, E::Err>
where
    E: Platform,
{
    let mut map = HashMap::new();
    for (point, recipients) in shard_by_location(ripple.to.clone(), adjacent, registry).await? {
        let mut ripple = ripple.clone();
        ripple.variant.to = recipients;
        map.insert(point, ripple);
    }
    Ok(map)
}

 */

pub async fn ripple_to_singulars<E>(
    ripple: Wave<Ripple>,
    adjacent: &HashSet<Point>,
    registry: &Registry<E>,
) -> Result<Vec<Wave<SingularRipple>>, E::Err>
where
    E: Platform,
{
    let mut rtn = vec![];
    for port in to_ports(ripple.to.clone(), adjacent, registry).await? {
        let wave = ripple.as_single(port);
        rtn.push(wave)
    }
    Ok(rtn)
}

/*
pub async fn shard_by_location<E>(
    recipients: Recipients,
    adjacent: &HashMap<Point, StarStub>,
    registry: &Registry<E>,
) -> Result<HashMap<Point, Recipients>, E::Err>
where
    E: Platform,
{
    match recipients {
        Recipients::Single(single) => {
            let mut map = HashMap::new();
            let record = registry.locate(&single.point).await?;
            map.insert(record.location, Recipients::Single(single));
            Ok(map)
        }
        Recipients::Multi(multi) => {
            let mut map: HashMap<Point, Vec<Port>> = HashMap::new();
            unimplemented!()
            /*
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

 */

pub async fn to_ports<E>(
    recipients: Recipients,
    adjacent: &HashSet<Point>,
    registry: &Registry<E>,
) -> Result<Vec<Port>, E::Err>
where
    E: Platform,
{
    match recipients {
        Recipients::Single(single) => Ok(vec![single]),
        Recipients::Multi(multi) => Ok(multi.into_iter().map(|p| p).collect()),
        Recipients::Watchers(watch) => {
            unimplemented!();
        }
        Recipients::Stars => {
            let stars: Vec<Port> = adjacent.clone().into_iter().map(|p| p.to_port()).collect();
            Ok(stars)
        }
    }
}

#[derive(Clone)]
pub struct DiagnosticInterceptors<P>
where
    P: Platform,
{
    pub from_hyperway: broadcast::Sender<UltraWave>,
    pub to_gravity: broadcast::Sender<UltraWave>,
    pub to_hyperway: broadcast::Sender<Wave<Signal>>,
    pub start_layer_traversal_wave: broadcast::Sender<UltraWave>,
    pub start_layer_traversal: broadcast::Sender<Traversal<UltraWave>>,
    pub transport_endpoint: broadcast::Sender<UltraWave>,
    pub reflected_endpoint: broadcast::Sender<UltraWave>,
    pub assignment: broadcast::Sender<Assign>,
    pub err: broadcast::Sender<P::Err>,
}

impl<P> DiagnosticInterceptors<P>
where
    P: Platform,
{
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


