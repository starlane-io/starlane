use crate::driver::{
    Item, CoreSkel, Driver, DriverFactory, DriverLifecycleCall, DriverSkel, DriverStatus,
};
use crate::star::{LayerInjectionRouter, StarSkel};
use crate::{Platform, Registry};
use cosmic_api::command::request::create::{Create, PointFactoryU64, TemplateDef};
use cosmic_api::error::MsgErr;
use cosmic_api::id::id::{Kind, Layer, Point, Port, ToPoint, ToPort};
use cosmic_api::id::{StarSub, TraversalInjection};
use cosmic_api::substance::substance::Substance;
use cosmic_api::sys::{Assign, InterchangeKind, ControlPattern, Greet, Knock, AssignmentKind};
use cosmic_api::wave::Agent::Anonymous;
use cosmic_api::wave::{DirectedProto, RecipientSelector};
use cosmic_api::wave::{
    Agent, CoreBounce, DirectedHandler, InCtx, ProtoTransmitter, ProtoTransmitterBuilder,
    RootInCtx, Signal, UltraWave, Wave,
};
use cosmic_api::wave::{DirectedHandlerSelector, SetStrategy, TxRouter};
use cosmic_api::{Registration, State};
use cosmic_hyperlane::{AnonHyperAuthenticator, AnonHyperAuthenticatorAssignEndPoint, HyperAuthenticator, HyperConnectionErr, HyperGate, HyperGreeter, Hyperway, HyperwayExt, HyperwayInterchange, HyperwayStub, InterchangeGate};
use dashmap::DashMap;
use std::marker::PhantomData;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, Mutex, RwLock};
use cosmic_api::command::command::common::StateSrc;

pub struct ControlDriverFactory<P>
where
    P: Platform,
{
    phantom: PhantomData<P>,
}

impl<P> DriverFactory<P> for ControlDriverFactory<P>
where
    P: Platform,
{
    fn kind(&self) -> Kind {
        Kind::Control
    }

    fn create(&self, skel: DriverSkel<P>) -> Box<dyn Driver> {
        Box::new(ControlDriver::new(skel))
    }
}

impl<P> ControlDriverFactory<P>
where
    P: Platform,
{
    pub fn new() -> Self {
        Self {
            phantom: Default::default(),
        }
    }
}

#[derive(DirectedHandler)]
pub struct ControlDriver<P>
where
    P: Platform,
{
    states: Arc<DashMap<Point, Arc<RwLock<ControlState>>>>,
    skel: DriverSkel<P>,
    runner_tx: mpsc::Sender<ControlCall<P>>,
}
use cosmic_api::config::config::bind::RouteSelector;
use cosmic_api::parse::route_attribute;
use cosmic_api::particle::particle::{Details, Status, Stub};
use cosmic_api::util::log;
use cosmic_api::wave::ReflectedCore;

#[routes]
impl<P> ControlDriver<P>
where
    P: Platform,
{
    fn new(skel: DriverSkel<P>) -> Self {
        let states = Arc::new(DashMap::new());
        let runner_tx = ControlDriverRunner::new(skel.clone(), states.clone());
        Self {
            skel,
            states,
            runner_tx,
        }
    }

    #[route("Cmd<Bounce>")]
    pub async fn bounce(&self, ctx: InCtx<'_, ()>) -> Result<ReflectedCore, MsgErr> {
        let mut core = ReflectedCore::new();
        Ok(core)
    }
}

#[async_trait]
impl<P> Driver for ControlDriver<P>
where
    P: Platform,
{
    fn kind(&self) -> Kind {
        Kind::Control
    }

    async fn status(&self) -> DriverStatus {
        let (rtn, rtn_rx) = oneshot::channel();
        self.runner_tx.send(ControlCall::GetStatus(rtn)).await;
        rtn_rx.await.unwrap_or(DriverStatus::Unknown)
    }

    async fn lifecycle(&mut self, event: DriverLifecycleCall) -> Result<DriverStatus, MsgErr> {
        self.runner_tx.send(ControlCall::Lifecycle(event)).await;
        Ok(self.status().await)
    }

    async fn item(&self, point: &Point) -> Result<Box<dyn Item>, MsgErr> {
        let (rtn, mut rtn_rx) = oneshot::channel();
        self.runner_tx
            .send(ControlCall::GetCore {
                point: point.clone(),
                rtn,
            })
            .await;
        let core = rtn_rx.await??;
        let core = Box::new(core);
        Ok(core)
    }

    async fn assign(&self, assign: Assign) -> Result<(), MsgErr> {
        let (rtn, mut rtn_rx) = oneshot::channel();
        self.runner_tx
            .send(ControlCall::Assign { assign, rtn })
            .await;
        rtn_rx.await??;
        Ok(())
    }
}

pub enum ControlCall<P>
where
    P: Platform,
{
    Lifecycle(DriverLifecycleCall),
    GetCore {
        point: Point,
        rtn: oneshot::Sender<Result<ControlCore<P>, MsgErr>>,
    },
    FromExternal(UltraWave),
    Assign {
        assign: Assign,
        rtn: oneshot::Sender<Result<(), MsgErr>>,
    },
    GetStatus(oneshot::Sender<DriverStatus>),
}

pub struct ControlDriverRunner<P>
where
    P: Platform,
{
    pub skel: DriverSkel<P>,
    pub states: Arc<DashMap<Point, Arc<RwLock<ControlState>>>>,
    pub status: DriverStatus,
    pub external_router: Option<Arc<TxRouter>>,
    pub runner_tx: mpsc::Sender<ControlCall<P>>,
    pub runner_rx: mpsc::Receiver<ControlCall<P>>,
}

impl<P> ControlDriverRunner<P>
where
    P: Platform,
{
    fn new(
        skel: DriverSkel<P>,
        states: Arc<DashMap<Point, Arc<RwLock<ControlState>>>>,
    ) -> mpsc::Sender<ControlCall<P>> {
        let (tx, rx) = mpsc::channel(1024);
        let status = DriverStatus::Pending;
        let mut runner = Self {
            states,
            skel,
            status,
            external_router: None,
            runner_tx: tx.clone(),
            runner_rx: rx,
        };
        runner.start();

        tx
    }

    fn log<O, E: ToString>(&self, result: Result<O, E>) -> Result<O, E> {
        match result {
            Ok(o) => Ok(o),
            Err(err) => {
                self.skel.logger.error(err.to_string());
                Err(err)
            }
        }
    }

    fn start(mut self) {
        tokio::spawn(async move {
            while let Some(call) = self.runner_rx.recv().await {
                match call {
                    ControlCall::Lifecycle(call) => match call {
                        DriverLifecycleCall::Init => {
                            self.status = DriverStatus::Initializing;
                            match log(self.init().await) {
                                Ok(_) => {
                                    self.status = DriverStatus::Ready;
                                }
                                Err(err) => {
                                    self.status = DriverStatus::Panic;
                                    self.skel.logger.error(err.to_string());
                                }
                            }
                        }
                        DriverLifecycleCall::Shutdown => {}
                    },
                    ControlCall::GetCore { point, rtn } => {
                        rtn.send(self.log(self.core(&point)));
                    }
                    ControlCall::FromExternal(wave) => {
                        self.log(self.route(wave));
                    }
                    ControlCall::Assign { assign, rtn } => {
                        rtn.send(self.log(self.assign(assign)));
                    }
                    ControlCall::GetStatus(rtn) => {
                        rtn.send(self.status.clone());
                    }
                }
            }
        });
    }

    async fn init(&mut self) -> Result<(), MsgErr> {
        self.status = DriverStatus::Initializing;
        let point = self.skel.star_skel.point.push("controls").unwrap();
        let logger = self.skel.star_skel.logger.point(point.clone());
        //let logger = self.skel.star_skel.logger.clone();
        let remote_point_factory =
            Arc::new(PointFactoryU64::new(point.clone(), "control-".to_string()));
        let auth = AnonHyperAuthenticatorAssignEndPoint::new(remote_point_factory);
        let mut interchange = HyperwayInterchange::new(logger.clone());
        let hyperway = Hyperway::new(Point::remote_endpoint().to_port(), Agent::HyperUser);
        let mut hyperway_ext = hyperway.mount().await;
        interchange.add(hyperway);
        interchange.singular_to(Point::remote_endpoint().to_port());
        let interchange = Arc::new(interchange);
        let external_router = Arc::new(TxRouter::new(hyperway_ext.tx.clone()));
        self.external_router = Some(external_router.clone());

        let greeter = ControlGreeter {};

        let gate = Arc::new(InterchangeGate::new(auth, greeter,interchange, logger));
        {
            let runner_tx = self.runner_tx.clone();
            tokio::spawn(async move {
                while let Some(wave) = hyperway_ext.rx.recv().await {
                    runner_tx.send(ControlCall::FromExternal(wave)).await;
                }
            });
        }

        self.skel
            .star_skel
            .machine
            .api
            .add_interchange(
                InterchangeKind::Control(ControlPattern::Star(self.skel.point.clone())),
                gate.clone(),
            )
            .await?;

        if self.skel.star_skel.kind == StarSub::Machine {
            self.skel
                .star_skel
                .machine
                .api
                .add_interchange(InterchangeKind::Control(ControlPattern::Any), gate)
                .await?;
        }

        Ok(())
    }

    fn assign(&self, assign: Assign) -> Result<(), MsgErr> {
        if self.states.contains_key(&assign.details.stub.point) {
            return Err("already assigned to this star".into());
        }
        self.states.insert(
            assign.details.stub.point.clone(),
            Arc::new(RwLock::new(ControlState::new())),
        );
        Ok(())
    }

    fn route(&self, hop: UltraWave) -> Result<(), MsgErr> {
        let agent = hop.agent().clone();
        let hop = hop.to_signal()?;
        let transport = hop.unwrap_from_hop()?;
        if transport.agent != agent {
            return Err("control transport agent mismatch".into());
        }
        let core = self.core(&transport.to.point)?;
        let wave = transport.unwrap_from_transport()?;
        if *wave.agent() != agent {
            return Err("control transport agent mismatch".into());
        }
        core.route(wave);
        Ok(())
    }

    fn core(&self, point: &Point) -> Result<ControlCore<P>, MsgErr> {
        let state = self
            .states
            .get(point)
            .ok_or::<MsgErr>("could not find state for control core".into())?
            .value()
            .clone();
        let router = LayerInjectionRouter::new(
            self.skel.star_skel.clone(),
            point.clone().to_port().with_layer(Layer::Core),
        );
        let mut transmitter =
            ProtoTransmitterBuilder::new(Arc::new(router), self.skel.star_skel.exchanger.clone());
        transmitter.from = SetStrategy::Override(point.clone().to_port().with_layer(Layer::Core));

        let skel = CoreSkel::new(point.clone(), transmitter.build());
        Ok(ControlCore::new(skel, state))
    }
}

impl<P> ControlDriver<P> where P: Platform {}

#[derive(DirectedHandler)]
pub struct ControlCore<P>
where
    P: Platform,
{
    skel: CoreSkel<P>,
    state: Arc<RwLock<ControlState>>,
}

impl<P> ControlCore<P>
where
    P: Platform,
{
    pub fn new(skel: CoreSkel<P>, state: Arc<RwLock<ControlState>>) -> Self {
        Self { skel, state }
    }

    pub async fn route(&self, wave: UltraWave) {
        self.skel.transmitter.route(wave).await;
    }
}

#[routes]
impl<P> Item for ControlCore<P> where P: Platform {}

pub struct ControlState {}

impl ControlState {
    pub fn new() -> Self {
        Self {}
    }
}


#[derive(Clone)]
pub struct ControlAuth<P> where P: Platform{
    star: Port,
    point: Point,
    registry: Registry<P>,
    call_tx: mpsc::Sender<ControlCall<P>>
}

impl <P> ControlAuth<P> where P: Platform {
    async fn create(&self, agent: Agent) -> Result<Point,P::Err> {
        let index = self.registry.sequence(&self.point).await?;
        let point = self.point.push(format!("control-{}",index) )?;
        let registration = Registration {
            point: point.clone(),
            kind: Kind::Control,
            registry: Default::default(),
            properties: Default::default(),
            owner: agent.to_point()
        };
        self.registry.register(&registration).await?;

        let assign = Assign {
            kind: AssignmentKind::Create,

            details: Details {
                stub: Stub {
                    point: point.clone(),
                    kind: Kind::Control,
                    status: Status::Ready
                },
                properties: Default::default()
            },
            state: StateSrc::None
        };
        let mut ping = DirectedProto::ping();
        ping.agent(Agent::HyperUser);
        ping.to(self.star.clone());
        ping.from(self.point.clone().to_port());

        Ok(point.clone())
    }
}

#[async_trait]
impl <P> HyperAuthenticator for ControlAuth<P> where P: Platform {
    async fn auth(&self, knock: Knock) -> Result<HyperwayStub, HyperConnectionErr> {
        unimplemented!()
    }
}

#[derive(Clone)]
pub struct ControlGreeter{
}



#[async_trait]
impl HyperGreeter for ControlGreeter {
    async fn greet(&self, stub: HyperwayStub) -> Result<Greet,MsgErr> {
        unimplemented!()
    }
}