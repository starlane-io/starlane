use crate::driver::{Driver, DriverFactory, DriverCtx, DriverSkel, DriverStatus, ItemDirectedHandler, ItemHandler, ItemSkel, HyperDriverFactory, HyperSkel, DriverRunnerRequest, Item, ItemRouter};
use crate::star::{LayerInjectionRouter, StarSkel};
use crate::{PlatErr, Platform, Registry};
use cosmic_api::command::command::common::StateSrc;
use cosmic_api::command::request::create::{Create, KindTemplate, PointFactory, PointFactoryU64, PointSegTemplate, PointTemplate, Strategy, Template, TemplateDef};
use cosmic_api::error::MsgErr;
use cosmic_api::id::id::{BaseKind, Kind, Layer, Point, Port, ToPoint, ToPort};
use cosmic_api::id::{StarSub, TraversalInjection};
use cosmic_api::substance::substance::Substance;
use cosmic_api::sys::{Assign, AssignmentKind, ControlPattern, Greet, InterchangeKind, Knock};
use cosmic_api::wave::Agent::Anonymous;
use cosmic_api::wave::{Agent, CoreBounce, DirectedHandler, InCtx, Pong, ProtoTransmitter, ProtoTransmitterBuilder, RootInCtx, Router, Signal, UltraWave, Wave};
use cosmic_api::wave::{DirectedHandlerSelector, SetStrategy, TxRouter};
use cosmic_api::wave::{DirectedProto, RecipientSelector};
use cosmic_api::{ArtRef, Registration, State};
use cosmic_hyperlane::{
    AnonHyperAuthenticator, AnonHyperAuthenticatorAssignEndPoint, HyperAuthenticator,
    HyperConnectionErr, HyperGate, HyperGreeter, Hyperway, HyperwayExt, HyperwayInterchange,
    HyperwayStub, InterchangeGate,
};
use dashmap::DashMap;
use std::marker::PhantomData;
use std::str::FromStr;
use std::sync::Arc;
use dashmap::mapref::one::Ref;
use tokio::sync::{mpsc, oneshot, Mutex, RwLock};

pub struct ControlDriverFactory<P>
where
    P: Platform,
{
    phantom: PhantomData<P>,
}

#[async_trait]
impl<P> HyperDriverFactory<P> for ControlDriverFactory<P>
where
    P: Platform,
{
    fn kind(&self) -> Kind {
        Kind::Control
    }

    async fn create(&self, star: StarSkel<P>, driver: DriverSkel<P>, ctx: DriverCtx) -> Result<Box<dyn Driver<P>>, P::Err> {
        let skel = HyperSkel::new( star, driver );
        Ok(Box::new(ControlDriver {
            skel,
            external_router: None,
            control_ctxs: Arc::new(Default::default()),
            fabric_routers: Arc::new(Default::default()),
            ctx,
        }))
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


use cosmic_api::config::config::bind::{BindConfig, RouteSelector};
use cosmic_api::log::{Track, Tracker};
use cosmic_api::parse::route_attribute;
use cosmic_api::particle::particle::{Details, Status, Stub};
use cosmic_api::util::log;
use cosmic_api::wave::ReflectedCore;

pub struct ControlFactory<P> where P: Platform {
   phantom: PhantomData<P>
}

impl <P> ControlFactory<P> where P: Platform {
    pub fn new() -> Self {
        Self {
            phantom: Default::default()
        }
    }
}

#[async_trait]
impl <P> HyperDriverFactory<P> for ControlFactory<P> where P: Platform{
    fn kind(&self) -> Kind {
        Kind::Control
    }

    async fn create(&self, star: StarSkel<P>, driver: DriverSkel<P>, ctx: DriverCtx) -> Result<Box<dyn Driver<P>>, P::Err> {
        let skel = HyperSkel::new(star,driver);

        Ok(Box::new(ControlDriver { skel, external_router: None, control_ctxs: Arc::new(Default::default()), fabric_routers: Arc::new(Default::default()), ctx }))
    }
}

#[derive(DirectedHandler)]
pub struct ControlDriver<P> where P: Platform {
    pub ctx: DriverCtx,
    pub skel: HyperSkel<P>,
    pub external_router: Option<TxRouter>,
    pub control_ctxs: Arc<DashMap<Point,ControlCtx<P>>>,
    pub fabric_routers: Arc<DashMap<Point,LayerInjectionRouter<P>>>
}

#[derive(Clone)]
pub struct ControlSkel<P> where P: Platform {
    pub star: StarSkel<P>,
    pub driver: DriverSkel<P>
}

#[routes]
impl<P> ControlDriver<P>
where
    P: Platform,
{
    #[route("Cmd<Bounce>")]
    pub async fn bounce(&self, ctx: InCtx<'_, ()>) -> Result<ReflectedCore, MsgErr> {
        let mut core = ReflectedCore::new();
        Ok(core)
    }
}



#[async_trait]
impl<P> Driver<P> for ControlDriver<P>
where
    P: Platform,
{
    fn kind(&self) -> Kind {
        Kind::Control
    }

    async fn init(&mut self, skel: DriverSkel<P>, ctx: DriverCtx) -> Result<(), P::Err> {
        self.skel.driver.status_tx.send(DriverStatus::Init).await;
        let point = skel.point.clone();
        let remote_point_factory =
            Arc::new(ControlCreator::new( self.skel.clone(), self.fabric_routers.clone(), ctx ));
        let auth = AnonHyperAuthenticatorAssignEndPoint::new(remote_point_factory, self.skel.driver.logger.clone() );
        let mut interchange = HyperwayInterchange::new(self.skel.driver.logger.clone());
        let hyperway = Hyperway::new(Point::remote_endpoint().to_port(), Agent::HyperUser);
        let ( tx, mut rx ) = hyperway.channel().await;
        interchange.add(hyperway);
        interchange.singular_to(Point::remote_endpoint().to_port());
        let interchange = Arc::new(interchange);
        let greeter = ControlGreeter::new(self.skel.clone(), self.skel.driver.point.push("controls".to_string()).unwrap());
        self.external_router  = Some(TxRouter::new(tx));
        let gate = Arc::new(InterchangeGate::new(auth, greeter, interchange, self.skel.driver.logger.clone() ));
        {
            let logger = self.skel.driver.logger.clone();
            let fabric_routers = self.fabric_routers.clone();
            tokio::spawn(async move {
                while let Some(hop) = rx.recv().await {
                    let remote = hop.from().clone().with_layer(Layer::Core);
                    match fabric_routers.get(&remote.point)
                    {
                        None => {
                            logger.warn("control not found");
                        }
                        Some(router) => {
                            let router = router.value();
                            match hop.unwrap_from_hop() {
                                Ok(transport) => {
                                    if transport.to.point == remote.point {
                                        match transport.unwrap_from_transport()
                                        {
                                            Ok(wave) => {
                                                router.route(wave).await;
                                            }
                                            Err(err) => {
                                                logger.warn(format!("could not unwrap from Transport: {}", err.to_string()));
                                            }
                                        }
                                    } else {
                                        logger.warn("remote control cannot transport  to any other point than its remote self".to_string());
                                    }
                                }
                                Err(err) => {
                                    logger.warn(format!("could not unwrap from Hop: {}", err.to_string()));
                                }
                            }
                        }
                    }

                }
            });
        }

        self.skel
            .star
            .machine
            .api
            .add_interchange(
                InterchangeKind::Control(ControlPattern::Star(self.skel.star.point.clone())),
                gate.clone(),
            )
            .await?;

        if self.skel.star.kind == StarSub::Machine {
            self.skel
                .star
                .machine
                .api
                .add_interchange(InterchangeKind::DefaultControl, gate)
                .await?;
        }

        self.skel.driver.status_tx.send(DriverStatus::Ready).await;

        Ok(())
    }

    async fn item(&self, point: &Point) -> Result<ItemHandler<P>, P::Err> {
        todo!()
    }


}

pub struct ControlCreator<P> where P: Platform {
   pub skel: HyperSkel<P>,
   pub fabric_routers: Arc<DashMap<Point,LayerInjectionRouter<P>>>,
   pub controls: Point,
   pub ctx: DriverCtx,
}

impl <P> ControlCreator<P> where P: Platform {
    pub fn new(skel: HyperSkel<P>, fabric_routers: Arc<DashMap<Point,LayerInjectionRouter<P>>>, ctx: DriverCtx) -> Self {
        let controls = skel.driver.point.push("controls").unwrap();
        Self {
            skel,
            fabric_routers,
            controls,
            ctx,
        }
    }
}

#[async_trait]
impl <P> PointFactory for ControlCreator<P> where P: Platform {
    async fn create(&self) -> Result<Point, MsgErr> {
println!("POINT FACTORY CREATE");
        let create = Create {
            template: Template::new( PointTemplate { parent:self.controls.clone(), child_segment_template: PointSegTemplate::Pattern("control-%".to_string())}, KindTemplate{ base: BaseKind::Control, sub: None, specific: None }),
            properties: Default::default(),
            strategy: Strategy::Commit,
            state: StateSrc::None,
        };
        let mut wave = create.to_wave_proto();
        wave.from(self.skel.driver.point.clone().to_port().with_layer(Layer::Core ));
        wave.agent(Agent::Point(self.skel.driver.point.clone()));

        self.skel.driver.logger.track(&wave, || Tracker::new("driver:control:creator", "Register"));


        let pong: Wave<Pong> = self.ctx.transmitter.direct(wave).await?;


        if pong.core.status.is_success() {
            if let Substance::Stub(ref stub) = pong.core.body {
                let point = stub.point.clone();
println!("~~ SUCCESS RTN STUB: {}",point.to_string());
                let fabric_router = LayerInjectionRouter::new(self.skel.star.clone(), point.clone().to_port().with_layer(Layer::Core) );
                self.fabric_routers.insert(point.clone(),fabric_router);
                Ok(point)
            }
            else {
println!("~~ FAIL .....STUB:" );
                Err(MsgErr::bad_request())
            }
        } else {
            Err(MsgErr::from_status(pong.core.status.as_u16()))
        }
    }
}


#[derive(Clone)]
pub struct ControlGreeter<P> where P: Platform{
    pub skel: HyperSkel<P>,
    pub controls: Point
}

impl <P> ControlGreeter<P> where P: Platform {
    pub fn new( skel: HyperSkel<P>, controls: Point ) -> Self {
        Self {
            skel,
            controls
        }
    }
}



#[async_trait]
impl <P> HyperGreeter for ControlGreeter<P> where P: Platform{
    async fn greet(&self, stub: HyperwayStub) -> Result<Greet,MsgErr> {
println!("GREETING!");
        Ok(Greet {
            port: stub.remote.clone(),
            agent: stub.agent.clone(),
            hop: Point::remote_endpoint().to_port().with_layer(Layer::Core),
            transport: stub.remote.clone()
        })
    }
}


pub struct Control<P> where P:Platform{
   pub skel: HyperSkel<P>,
   pub ctx: ControlCtx<P>
}

impl <P> Item<P> for Control<P> where P: Platform{
    type Skel = HyperSkel<P>;
    type Ctx = ControlCtx<P>;
    type State = ();

    fn restore(skel: Self::Skel, ctx: Self::Ctx, _: Self::State) -> Self {
        Self {
            skel,
            ctx
        }
    }
}

#[async_trait]
impl <P> Router for Control<P> where P: Platform {
    async fn route(&self, wave: UltraWave) {
        self.ctx.router.route(wave).await;
    }

    fn route_sync(&self, wave: UltraWave) {
        self.ctx.router.route_sync(wave);
    }
}

#[async_trait]
impl <P> ItemRouter<P> for Control<P> where P: Platform {
    async fn bind(&self) -> Result<ArtRef<BindConfig>, P::Err> {
        <Control<P> as Item<P>>::bind(self).await
    }
}

#[derive(Clone)]
pub struct ControlCtx<P> where P: Platform {
   pub phantom: PhantomData<P>,
   pub router: TxRouter
}

impl <P> ControlCtx<P> where P: Platform {
    pub fn new(tx: mpsc::Sender<UltraWave>) -> Self {
        let router = TxRouter::new(tx);
        Self {
            phantom: Default::default(),
            router
        }
    }
}



/*
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

        let skel = ItemSkel::new(point.clone(), transmitter.build());
        Ok(ControlCore::new(skel, state))
    }
}

impl<P> ControlDriver<P> where P: Platform {}

#[derive(DirectedHandler)]
pub struct ControlCore<P>
where
    P: Platform,
{
    skel: ItemSkel<P>,
    state: Arc<RwLock<ControlState>>,
}

impl<P> ControlCore<P>
where
    P: Platform,
{
    pub fn new(skel: ItemSkel<P>, state: Arc<RwLock<ControlState>>) -> Self {
        Self { skel, state }
    }

    pub async fn route(&self, wave: UltraWave) {
        self.skel.transmitter.route(wave).await;
    }
}

#[routes]
impl<P> Item<P> for ControlCore<P> where P: Platform {
    type Skel = ();
    type State = ();
    type Ctx = ();

    fn restore(skel: ItemSkel<P>, ctx: Self::Ctx, state: Self::State) -> Self {
        todo!()
    }
}

pub struct ControlContext<P> where P: Platform {

}

impl <P> ControlContext<P> where P: Platform  {

}

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

 */
