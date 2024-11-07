use crate::driver::{
    Driver, DriverAvail, DriverCtx, DriverErr, DriverSkel, DriverStatus, HyperDriverFactory,
    HyperSkel, Particle, ParticleErr, ParticleRouter, ParticleSphere, ParticleSphereInner,
};
use crate::hyperlane::{
    AnonHyperAuthenticatorAssignEndPoint, FromTransform, HopTransform, HyperClient, HyperGreeter,
    Hyperway, HyperwayConfigurator, HyperwayEndpointFactory, HyperwayInterchange, HyperwayStub,
    InterchangeGate, TransportTransform,
};
use crate::star::{HyperStarSkel, LayerInjectionRouter};
use crate::platform::Platform;
use dashmap::DashMap;
use starlane::space::artifact::ArtRef;
use starlane::space::command::common::StateSrc;
use starlane::space::command::direct::create::{
    Create, KindTemplate, PointSegTemplate, PointTemplate, Strategy, Template,
};
use starlane::space::command::RawCommand;
use starlane::space::config::bind::BindConfig;
use starlane::space::err::{CoreReflector, SpaceErr};
use starlane::space::hyper::{ControlPattern, Greet, InterchangeKind};
use starlane::space::kind::{BaseKind, Kind, StarSub};
use starlane::space::loc::{Layer, PointFactory, Surface, ToSurface};
use starlane::space::particle::traversal::Traversal;
use starlane::space::point::Point;
use starlane::space::selector::KindSelector;
use starlane::space::settings::Timeouts;
use starlane::space::substance::{Substance, SubstanceErr};
use starlane::space::wave::core::ext::ExtMethod;
use starlane::space::wave::core::ReflectedCore;
use starlane::space::wave::exchange::asynch::{
    Exchanger, ProtoTransmitter, ProtoTransmitterBuilder, Router, TraversalRouter,
};
use starlane::space::wave::exchange::SetStrategy;
use starlane::space::wave::{Agent, DirectedProto, PongCore, ToRecipients, Wave, WaveVariantDef};
pub use starlane_space as starlane;
use std::marker::PhantomData;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use anyhow::anyhow;
use thiserror::Error;
use starlane::space::log::Tracker;
use starlane_primitive_macros::logger;

pub struct ControlDriverFactory {}

#[async_trait]
impl HyperDriverFactory for ControlDriverFactory {
    fn kind(&self) -> Kind {
        Kind::Control
    }

    fn selector(&self) -> KindSelector {
        KindSelector::from_base(BaseKind::Control)
    }

    fn avail(&self) -> DriverAvail {
        DriverAvail::Internal
    }

    async fn create(
        &self,
        star: HyperStarSkel,
        driver: DriverSkel,
        ctx: DriverCtx,
    ) -> Result<Box<dyn Driver>, DriverErr> {
        let skel = HyperSkel::new(star, driver);
        Ok(Box::new(ControlDriver {
            skel,
            external_router: None,
            control_ctxs: Arc::new(Default::default()),
            fabric_routers: Arc::new(Default::default()),
            ctx,
        }))
    }
}

impl ControlDriverFactory {
    pub fn new() -> Self {
        Self {}
    }
}

pub struct ControlFactory {}

impl ControlFactory {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl HyperDriverFactory for ControlFactory {
    fn kind(&self) -> Kind {
        Kind::Control
    }

    fn selector(&self) -> KindSelector {
        KindSelector::from_base(BaseKind::Control)
    }

    async fn create(
        &self,
        star: HyperStarSkel,
        driver: DriverSkel,
        ctx: DriverCtx,
    ) -> Result<Box<dyn Driver>, DriverErr> {
        let skel = HyperSkel::new(star, driver);

        Ok(Box::new(ControlDriver {
            skel,
            external_router: None,
            control_ctxs: Arc::new(Default::default()),
            fabric_routers: Arc::new(Default::default()),
            ctx,
        }))
    }
}

#[derive(DirectedHandler)]
pub struct ControlDriver {
    pub ctx: DriverCtx,
    pub skel: HyperSkel,
    pub external_router: Option<Arc<dyn Router>>,
    pub control_ctxs: Arc<DashMap<Point, ControlCtx>>,
    pub fabric_routers: Arc<DashMap<Point, LayerInjectionRouter>>,
}

#[derive(Clone)]
pub struct ControlSkel {
    pub star: HyperStarSkel,
    pub driver: DriverSkel,
}

#[async_trait]
impl Driver for ControlDriver {
    fn kind(&self) -> Kind {
        Kind::Control
    }

    fn layer(&self) -> Layer {
        Layer::Portal
    }

    async fn init(&mut self, skel: DriverSkel, ctx: DriverCtx) -> Result<(), DriverErr> {
        self.skel.driver.status_tx.send(DriverStatus::Init).await;

        skel.create_in_driver(
            PointSegTemplate::Exact("controls".to_string()),
            Kind::Base.to_template(),
        )
        .await?;

        let remote_point_factory = Arc::new(ControlCreator::new(
            self.skel.clone(),
            self.fabric_routers.clone(),
            ctx,
        ));
        let auth = AnonHyperAuthenticatorAssignEndPoint::new(
            remote_point_factory,
            self.skel.driver.logger.clone(),
        );
        let point = skel.point.clone();
        let mut interchange = HyperwayInterchange::new(point,self.skel.driver.logger.clone());
        let hyperway = Hyperway::new(
            Point::remote_endpoint().to_surface(),
            Agent::HyperUser,
            self.skel.driver.logger.clone(),
        );
        let mut hyperway_endpoint = hyperway.hyperway_endpoint_far(None).await;
        interchange.add(hyperway).await;
        interchange.singular_to(Point::remote_endpoint().to_surface());
        let interchange = Arc::new(interchange);
        let greeter = ControlGreeter::new(
            self.skel.clone(),
            self.skel.driver.point.push("controls".to_string()).unwrap(),
        );
        self.external_router = Some(interchange.router().into());

        pub struct ControlHyperwayConfigurator;

        impl HyperwayConfigurator for ControlHyperwayConfigurator {
            fn config(&self, greet: &Greet, hyperway: &mut Hyperway) {
                hyperway.transform_inbound(Box::new(FromTransform::new(greet.surface.clone())));
                hyperway
                    .transform_inbound(Box::new(TransportTransform::new(greet.transport.clone())));
                hyperway.transform_inbound(Box::new(HopTransform::new(greet.hop.clone())));
            }
        }
        let configurator = ControlHyperwayConfigurator;
        let gate = Arc::new(InterchangeGate::new(
            auth,
            greeter,
            configurator,
            interchange,
            self.skel.driver.logger.clone(),
        ));
        {
            let logger = self.skel.driver.logger.clone();
            let fabric_routers = self.fabric_routers.clone();
            let skel = self.skel.clone();
            tokio::spawn(async move {
                while let Some(hop) = hyperway_endpoint.rx.recv().await {
                    let remote = hop.from().clone().with_layer(Layer::Portal);
                    match fabric_routers.get(&remote.point) {
                        None => {
                            logger.warn("control not found");
                        }
                        Some(router) => {
                            let injector = remote.with_layer(Layer::Shell);
                            let router = LayerInjectionRouter::new(skel.star.clone(), injector);

                            match hop.unwrap_from_hop() {
                                Ok(transport) => {
                                    if transport.to.point == remote.point {
                                        match transport.unwrap_from_transport() {
                                            Ok(mut wave) => {
                                                router.route(wave).await;
                                            }
                                            Err(err) => {
                                                logger.warn(format!(
                                                    "could not unwrap from Transport: {}",
                                                    err.to_string()
                                                ));
                                            }
                                        }
                                    } else {
                                        logger.warn("remote control cannot transport  to any other point than its remote self".to_string());
                                    }
                                }
                                Err(err) => {
                                    logger.warn(format!(
                                        "could not unwrap from Hop: {}",
                                        err.to_string()
                                    ));
                                }
                            }
                        }
                    }
                }
            });
        }

        self.skel
            .star
            .machine_api
            .add_interchange(
                InterchangeKind::Control(ControlPattern::Star(self.skel.star.point.clone())),
                gate.clone(),
            )
            .await?;

        if self.skel.star.kind == StarSub::Machine {
            self.skel
                .star
                .machine_api
                .add_interchange(InterchangeKind::DefaultControl, gate)
                .await?;
        }

        self.skel.driver.status_tx.send(DriverStatus::Ready).await;

        Ok(())
    }

    async fn particle(&self, point: &Point) -> Result<ParticleSphere, DriverErr> {
        let router = self
            .external_router
            .as_ref()
            .ok_or(DriverErr::particle_router_not_set(point, &self.kind()))?
            .clone();
        let ctx = ControlCtx::new(router);
        let control = Control::restore(self.skel.clone(), ctx, ());

        Ok(control.sphere()?)
    }
}

pub struct ControlCreator {
    pub skel: HyperSkel,
    pub fabric_routers: Arc<DashMap<Point, LayerInjectionRouter>>,
    pub controls: Point,
    pub ctx: DriverCtx,
}

impl ControlCreator {
    pub fn new(
        skel: HyperSkel,
        fabric_routers: Arc<DashMap<Point, LayerInjectionRouter>>,
        ctx: DriverCtx,
    ) -> Self {
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
impl PointFactory for ControlCreator {
    async fn create(&self) -> Result<Point, SpaceErr> {
        let create = Create {
            template: Template::new(
                PointTemplate {
                    parent: self.controls.clone(),
                    child_segment_template: PointSegTemplate::Pattern("control-%".to_string()),
                },
                KindTemplate {
                    base: BaseKind::Control,
                    sub: None,
                    specific: None,
                },
            ),
            properties: Default::default(),
            strategy: Strategy::Commit,
            state: StateSrc::None,
        };

        match self.skel.driver.logger.result_ctx(
            "create-control",
            self.skel.star.create_in_star(create).await,
        ) {
            Ok(details) => {
                let point = details.stub.point;
                let fabric_router = LayerInjectionRouter::new(
                    self.skel.star.clone(),
                    point.clone().to_surface().with_layer(Layer::Shell),
                );
                self.fabric_routers.insert(point.clone(), fabric_router);
                Ok(point)
            }
            Err(err) => Err(anyhow!(err))?,
        }
    }
}

#[derive(Clone)]
pub struct ControlGreeter {
    pub skel: HyperSkel,
    pub controls: Point,
}

impl ControlGreeter {
    pub fn new(skel: HyperSkel, controls: Point) -> Self {
        Self { skel, controls }
    }
}

#[async_trait]
impl HyperGreeter for ControlGreeter {
    async fn greet(&self, stub: HyperwayStub) -> Result<Greet, SpaceErr> {
        Ok(Greet {
            surface: stub.remote.clone().with_layer(Layer::Core),
            agent: stub.agent.clone(),
            hop: self.skel.driver.point.clone().to_surface(),
            transport: stub.remote.clone().with_layer(Layer::Portal),
        })
    }
}

pub struct Control {
    pub skel: HyperSkel,
    pub ctx: ControlCtx,
}

impl Particle for Control {
    type Skel = HyperSkel;
    type Ctx = ControlCtx;
    type State = ();
    type Err = ControlErr;

    fn restore(skel: Self::Skel, ctx: Self::Ctx, _: Self::State) -> Self {
        Self { skel, ctx }
    }

    fn sphere(self) -> Result<ParticleSphere, Self::Err> {
        Ok(ParticleSphere::new_router(self))
    }
}

#[async_trait]
impl TraversalRouter for Control {
    async fn traverse(&self, traversal: Traversal<Wave>) -> Result<(), SpaceErr> {
        self.skel.driver.logger.track(&traversal, || {
            Tracker::new(
                format!("control -> {}", traversal.dir.to_string()),
                "Traverse",
            )
        });

        self.ctx.router.route(traversal.payload).await;
        Ok(())
    }
}

#[derive(Clone)]
pub struct ControlCtx {
    pub router: Arc<dyn Router>,
}

impl ControlCtx {
    pub fn new(router: Arc<dyn Router>) -> Self {
        Self { router }
    }
}

pub struct ControlClient {
    client: HyperClient,
}

impl ControlClient {
    pub fn new(factory: Box<dyn HyperwayEndpointFactory>) -> Result<Self, SpaceErr> {
        let exchanger = Exchanger::new(
            Point::from_str("control-client")?.to_surface(),
            Timeouts::default(),
            Default::default(),
        );
        let logger = logger!(Point::from_str("control-client")?);
        let client = HyperClient::new_with_exchanger(factory, Some(exchanger), logger)?;
        Ok(Self { client })
    }

    pub fn surface(&self) -> Result<Surface, SpaceErr> {
        let greet = self
            .client
            .get_greeting()
            .ok_or("cannot access surface until greeting has been received")?;
        Ok(greet.surface)
    }

    pub async fn wait_for_ready(&self, duration: Duration) -> Result<(), SpaceErr> {
        self.client.wait_for_ready(duration).await
    }

    pub async fn wait_for_greet(&self) -> Result<Greet, SpaceErr> {
        self.client.wait_for_greet().await
    }

    pub async fn transmitter_builder(&self) -> Result<ProtoTransmitterBuilder, SpaceErr> {
        self.client.transmitter_builder().await
    }

    pub async fn new_cli_session(&self) -> Result<ControlCliSession, SpaceErr> {
        let transmitter = self.transmitter_builder().await?.build();
        let mut proto = DirectedProto::ping();
        proto.to(self.surface()?.with_layer(Layer::Shell));
        proto.method(ExtMethod::new("NewCliSession".to_string())?);
        let pong: WaveVariantDef<PongCore> = transmitter.direct(proto).await?;
        pong.ok_or()?;
        if let Substance::Surface(port) = pong.variant.core.body {
            let mut transmitter = self.transmitter_builder().await?;
            transmitter.to = SetStrategy::Override(port.to_recipients());
            let transmitter = transmitter.build();
            Ok(ControlCliSession::new(transmitter))
        } else {
            Err("NewCliSession expected: Surface".into())
        }
    }
}

pub struct ControlCliSession {
    transmitter: ProtoTransmitter,
}

impl ControlCliSession {
    pub fn new(transmitter: ProtoTransmitter) -> Self {
        Self { transmitter }
    }
    pub async fn exec<C>(&self, command: C) -> Result<ReflectedCore, SpaceErr>
    where
        C: ToString,
    {
        let command = RawCommand::new(command.to_string());
        self.raw(command).await
    }

    pub async fn raw(&self, command: RawCommand) -> Result<ReflectedCore, SpaceErr> {
        let mut proto = DirectedProto::ping();
        proto.method(ExtMethod::new("Exec".to_string())?);
        proto.body(Substance::RawCommand(command));
        let pong: WaveVariantDef<PongCore> = self.transmitter.direct(proto).await?;
        Ok(pong.variant.core)
    }
}

#[derive(Error, Debug, Clone)]
pub enum ControlErr {
    #[error(transparent)]
    SpaceErr(#[from] SpaceErr),
}



impl CoreReflector for ControlErr {
    fn as_reflected_core(self) -> ReflectedCore {
        ReflectedCore {
            headers: Default::default(),
            status: Default::default(),
            body: Substance::Err(SubstanceErr(format!("{}",self.to_string())))
        }
    }
}

impl ParticleErr for ControlErr {}
