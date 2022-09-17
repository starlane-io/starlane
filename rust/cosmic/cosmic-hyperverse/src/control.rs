use crate::driver::{Driver, DriverAvail, DriverCtx, DriverFactory, DriverRunnerRequest, DriverSkel, DriverStatus, HyperDriverFactory, HyperSkel, Item, ItemHandler, ItemRouter, ItemSkel, ItemSphere};
use crate::star::{HyperStarSkel, LayerInjectionRouter};
use crate::{PlatErr, Platform, Registry};
use cosmic_universe::command::common::StateSrc;
use cosmic_universe::command::request::create::{Create, KindTemplate, PointFactory, PointFactoryU64, PointSegTemplate, PointTemplate, Strategy, Template, TemplateDef};
use cosmic_universe::error::UniErr;
use cosmic_universe::substance2::substance::Substance;
use cosmic_universe::hyper::{Assign, AssignmentKind, ControlPattern, Greet, InterchangeKind, Knock};
use cosmic_universe::wave::Agent::Anonymous;
use cosmic_universe::wave::{Agent, CmdMethod, CoreBounce, DirectedHandler, Exchanger, InCtx, Method, Pong, ProtoTransmitter, ProtoTransmitterBuilder, RootInCtx, Router, Signal, ToRecipients, UltraWave, Wave};
use cosmic_universe::wave::{DirectedHandlerSelector, SetStrategy, TxRouter};
use cosmic_universe::wave::{DirectedProto, RecipientSelector};
use cosmic_universe::reg::Registration;
use cosmic_hyperlane::{AnonHyperAuthenticator, AnonHyperAuthenticatorAssignEndPoint, FromTransform, HopTransform, HyperAuthenticator, HyperClient, HyperConnectionErr, HyperGate, HyperGreeter, Hyperway, HyperwayConfigurator, HyperwayEndpoint, HyperwayEndpointFactory, HyperwayInterchange, HyperwayStub, InterchangeGate, TransportTransform};
use dashmap::DashMap;
use std::marker::PhantomData;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use dashmap::mapref::one::Ref;
use tokio::sync::{mpsc, Mutex, oneshot, RwLock};
use cosmic_universe::artifact::ArtRef;
use cosmic_universe::cli::RawCommand;

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

    fn avail(&self) -> DriverAvail {
        DriverAvail::Internal
    }

    async fn create(&self, star: HyperStarSkel<P>, driver: DriverSkel<P>, ctx: DriverCtx) -> Result<Box<dyn Driver<P>>, P::Err> {
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


use cosmic_universe::config::bind::{BindConfig, RouteSelector};
use cosmic_universe::log::{RootLogger, Track, Tracker};
use cosmic_universe::ext::ExtMethod;
use cosmic_universe::id::{BaseKind, Kind, Layer, Point, Port, StarSub, ToPoint, ToPort, TraversalInjection};
use cosmic_universe::parse::{CamelCase, route_attribute};
use cosmic_universe::particle2::particle::{Details, Status, Stub};
use cosmic_universe::quota::Timeouts;
use cosmic_universe::state::State;
use cosmic_universe::util::log;
use cosmic_universe::wave::ReflectedCore;
use crate::star::HyperStarCall::LayerTraversalInjection;

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

    async fn create(&self, star: HyperStarSkel<P>, driver: DriverSkel<P>, ctx: DriverCtx) -> Result<Box<dyn Driver<P>>, P::Err> {
        let skel = HyperSkel::new(star,driver);

        Ok(Box::new(ControlDriver { skel, external_router: None, control_ctxs: Arc::new(Default::default()), fabric_routers: Arc::new(Default::default()), ctx }))
    }
}

#[derive(DirectedHandler)]
pub struct ControlDriver<P> where P: Platform {
    pub ctx: DriverCtx,
    pub skel: HyperSkel<P>,
    pub external_router: Option<Arc<dyn Router>>,
    pub control_ctxs: Arc<DashMap<Point,ControlCtx<P>>>,
    pub fabric_routers: Arc<DashMap<Point,LayerInjectionRouter>>,
}

#[derive(Clone)]
pub struct ControlSkel<P> where P: Platform {
    pub star: HyperStarSkel<P>,
    pub driver: DriverSkel<P>
}

#[routes]
impl<P> ControlDriver<P>
where
    P: Platform,
{

}



#[async_trait]
impl<P> Driver<P> for ControlDriver<P>
where
    P: Platform,
{
    fn kind(&self) -> Kind {
        Kind::Control
    }

    fn layer(&self) -> Layer {
        Layer::Portal
    }

    async fn init(&mut self, skel: DriverSkel<P>, ctx: DriverCtx) -> Result<(), P::Err> {
        self.skel.driver.status_tx.send(DriverStatus::Init).await;

        skel.create_driver_particle(PointSegTemplate::Exact("controls".to_string()), Kind::Base.to_template()).await?;

        let remote_point_factory =
            Arc::new(ControlCreator::new( self.skel.clone(), self.fabric_routers.clone(), ctx ));
        let auth = AnonHyperAuthenticatorAssignEndPoint::new(remote_point_factory, self.skel.driver.logger.clone() );
        let mut interchange = HyperwayInterchange::new(self.skel.driver.logger.clone());
        let hyperway = Hyperway::new(Point::remote_endpoint().to_port(), Agent::HyperUser);
        let mut hyperway_endpoint = hyperway.hyperway_endpoint_far(None).await;
        interchange.add(hyperway).await;
        interchange.singular_to(Point::remote_endpoint().to_port());
        let interchange = Arc::new(interchange);
        let greeter = ControlGreeter::new(self.skel.clone(), self.skel.driver.point.push("controls".to_string()).unwrap());
        self.external_router  = Some(interchange.router().into());

        pub struct ControlHyperwayConfigurator;

        impl HyperwayConfigurator for ControlHyperwayConfigurator {
            fn config(&self, greet: &Greet, hyperway: &mut Hyperway) {
                hyperway.transform_inbound( Box::new(FromTransform::new(greet.port.clone())) );
                hyperway.transform_inbound( Box::new(TransportTransform::new(greet.transport.clone())) );
                hyperway.transform_inbound( Box::new(HopTransform::new(greet.hop.clone())) );
            }
        }
        let configurator = ControlHyperwayConfigurator;
        let gate = Arc::new(InterchangeGate::new(auth, greeter, configurator, interchange, self.skel.driver.logger.clone() ));
        {
            let logger = self.skel.driver.logger.clone();
            let fabric_routers = self.fabric_routers.clone();
            let skel = self.skel.clone();
            tokio::spawn(async move {
                while let Some(hop) = hyperway_endpoint.rx.recv().await {
                    let remote = hop.from().clone().with_layer(Layer::Portal);
                    match fabric_routers.get(&remote.point)
                    {
                        None => {
                            logger.warn("control not found");
                        }
                        Some(router) => {
                            let injector = remote.with_layer(Layer::Shell );
                            let router = LayerInjectionRouter::new( skel.star.clone(), injector);

                            match hop.unwrap_from_hop() {
                                Ok(transport) => {
                                    if transport.to.point == remote.point {
                                        match transport.unwrap_from_transport()
                                        {
                                            Ok(mut wave) => {
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

    async fn item(&self, point: &Point) -> Result<ItemSphere<P>, P::Err> {
        let router = self.external_router.as_ref().ok_or(P::Err::new("FATAL: router is not set") )?.clone();
        let ctx = ControlCtx::new(router);
        Ok(ItemSphere::Router(Box::new(Control::restore(self.skel.clone(), ctx, ()))))
    }
}

pub struct ControlCreator<P> where P: Platform {
   pub skel: HyperSkel<P>,
   pub fabric_routers: Arc<DashMap<Point,LayerInjectionRouter>>,
   pub controls: Point,
   pub ctx: DriverCtx,
}

impl <P> ControlCreator<P> where P: Platform {
    pub fn new(skel: HyperSkel<P>, fabric_routers: Arc<DashMap<Point,LayerInjectionRouter>>, ctx: DriverCtx) -> Self {
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
    async fn create(&self) -> Result<Point, UniErr> {

        let create = Create {
            template: Template::new( PointTemplate { parent:self.controls.clone(), child_segment_template: PointSegTemplate::Pattern("control-%".to_string())}, KindTemplate{ base: BaseKind::Control, sub: None, specific: None }),
            properties: Default::default(),
            strategy: Strategy::Commit,
            state: StateSrc::None,
        };

        match self.skel.driver.logger.result_ctx("create-control",self.skel.star.create_in_star(create).await) {
            Ok(details) => {
                let point = details.stub.point;
                let fabric_router = LayerInjectionRouter::new(self.skel.star.clone(), point.clone().to_port().with_layer(Layer::Shell) );
                self.fabric_routers.insert(point.clone(),fabric_router);
                Ok(point)
            }
            Err(err) => {
                Err(err.into())
            }
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
    async fn greet(&self, stub: HyperwayStub) -> Result<Greet, UniErr> {
        Ok(Greet {
            port: stub.remote.clone().with_layer(Layer::Core),
            agent: stub.agent.clone(),
            hop: self.skel.driver.point.clone().to_port(),
            transport: stub.remote.clone().with_layer(Layer::Portal )
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
   pub router: Arc<dyn Router>
}

impl <P> ControlCtx<P> where P: Platform {
    pub fn new(router: Arc<dyn Router>) -> Self {
        Self {
            phantom: Default::default(),
            router
        }
    }
}




pub struct ControlClient {
    client: HyperClient
}

impl  ControlClient  {
    pub fn new(factory: Box<dyn HyperwayEndpointFactory>) -> Result<Self, UniErr>{
        let exchanger = Exchanger::new( Point::from_str("control-client")?.to_port(), Timeouts::default());
        let logger = RootLogger::default();
        let logger = logger.point(Point::from_str("control-client")?);
        let client = HyperClient::new_with_exchanger(factory, Some(exchanger), logger )?;
        Ok(Self {
            client
        })
    }

    pub fn port(&self) -> Result<Port, UniErr> {
        let greet = self.client.get_greeting().ok_or("cannot access port until greeting has been received")?;
        Ok(greet.port)
    }

    pub async fn wait_for_ready(&self, duration: Duration ) -> Result<(), UniErr>{
        self.client.wait_for_ready(duration).await
    }

    pub async fn transmitter_builder(&self) -> Result<ProtoTransmitterBuilder, UniErr> {
        self.client.transmitter_builder().await
    }

    pub async fn new_cli_session(&self) -> Result<ControlCliSession, UniErr> {
        let transmitter = self.transmitter_builder().await?.build();
        let mut proto = DirectedProto::ping();
        proto.to( self.port()?.with_layer(Layer::Shell));
        proto.method(ExtMethod::new("NewCliSession".to_string())?);
        let pong: Wave<Pong> = transmitter.direct(proto).await?;
        pong.ok_or()?;
        if let Substance::Port(port) = pong.variant.core.body {
            let mut transmitter = self.transmitter_builder().await?;
            transmitter.to = SetStrategy::Override(port.to_recipients());
            let transmitter = transmitter.build();
            Ok(ControlCliSession::new(transmitter))
        } else {
            Err("NewCliSession expected: Port".into())
        }
    }
}

pub struct ControlCliSession {
    transmitter: ProtoTransmitter
}

impl ControlCliSession {
    pub fn new( transmitter: ProtoTransmitter ) -> Self {
        Self {
            transmitter
        }
    }
    pub async fn exec<C>( &self, command: C) -> Result<ReflectedCore, UniErr> where C: ToString{
        let command = RawCommand::new(command.to_string());
        self.raw(command).await
    }

    pub async fn raw( &self, command: RawCommand ) -> Result<ReflectedCore, UniErr> {
        let mut proto = DirectedProto::ping();
        proto.method(ExtMethod::new("Exec".to_string())?);
        proto.body(Substance::RawCommand(command));
        let pong: Wave<Pong> = self.transmitter.direct(proto).await?;
        Ok(pong.variant.core)
    }
}

