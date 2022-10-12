use crate::driver::{
    Driver, DriverAvail, DriverCtx, DriverHandler, DriverSkel, DriverStatus, HyperDriverFactory,
    Item, ItemCtx, ItemHandler, ItemRouter, ItemSkel, ItemSphere,
};
use crate::err::HyperErr;
use crate::star::{HyperStarSkel, LayerInjectionRouter};
use crate::Cosmos;
use cosmic_space::artifact::ArtRef;
use cosmic_space::command::common::{PropertyMod, SetProperties, StateSrc};
use cosmic_space::command::direct::create::{
    Create, PointSegTemplate, PointTemplate, Strategy, Template, TemplateDef,
};
use cosmic_space::config::bind::BindConfig;
use cosmic_space::err::SpaceErr;
use cosmic_space::hyper::{Assign, HyperSubstance, ParticleLocation};
use cosmic_space::kind::{BaseKind, Kind};
use cosmic_space::loc::{Layer, Point, ToSurface};
use cosmic_space::log::RootLogger;
use cosmic_space::parse::bind_config;
use cosmic_space::particle::traversal::{Traversal, TraversalDirection};
use cosmic_space::selector::KindSelector;
use cosmic_space::substance::Substance;
use cosmic_space::util::log;
use cosmic_space::wave::core::hyp::HypMethod;
use cosmic_space::wave::core::DirectedCore;
use cosmic_space::wave::exchange::asynch::{InCtx, TraversalRouter};
use cosmic_space::wave::{DirectedProto, DirectedWave, Pong, UltraWave, Wave};
use dashmap::DashMap;
use mechtron_host::{HostPlatform, MechtronHost, MechtronHostFactory};
use std::marker::PhantomData;
use std::ops::Deref;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

lazy_static! {
    static ref HOST_DRIVER_BIND_CONFIG: ArtRef<BindConfig> = ArtRef::new(
        Arc::new(host_driver_bind()),
        Point::from_str("GLOBAL::repo:1.0.0:/bind/host-driver.bind").unwrap()
    );
    static ref HOST_BIND_CONFIG: ArtRef<BindConfig> = ArtRef::new(
        Arc::new(host_bind()),
        Point::from_str("GLOBAL::repo:1.0.0:/bind/host.bind").unwrap()
    );
    static ref MECHTRON_BIND_CONFIG: ArtRef<BindConfig> = ArtRef::new(
        Arc::new(mechtron_bind()),
        Point::from_str("GLOBAL::repo:1.0.0:/bind/mechtron.bind").unwrap()
    );
}

fn host_driver_bind() -> BindConfig {
    log(bind_config(
        r#"
    Bind(version=1.0.0)
    {
       Route -> {
           Hyp<Host> -> (()) => &;
           Hyp<Assign> -> (()) => &;
       }
    }
    "#,
    ))
    .unwrap()
}

fn host_bind() -> BindConfig {
    log(bind_config(
        r#"
    Bind(version=1.0.0)
    {
       Route -> {
          Hyp<Transport> -> (());
       }
    }
    "#,
    ))
    .unwrap()
}

fn mechtron_bind() -> BindConfig {
    log(bind_config(
        r#"
    Bind(version=1.0.0)
    {
       Route -> {
          Ext<*> -> (()) => &;
          Http<*> -> (()) => &;
       }
    }
    "#,
    ))
    .unwrap()
}

pub struct HostDriverFactory {}

impl HostDriverFactory {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl<P> HyperDriverFactory<P> for HostDriverFactory
where
    P: Cosmos,
{
    fn kind(&self) -> KindSelector {
        KindSelector::from_base(BaseKind::Host)
    }

    async fn create(
        &self,
        skel: HyperStarSkel<P>,
        driver_skel: DriverSkel<P>,
        ctx: DriverCtx,
    ) -> Result<Box<dyn Driver<P>>, P::Err> {
        Ok(Box::new(HostDriver::new(driver_skel, ctx)))
    }
}

pub struct HostDriver<P>
where
    P: Cosmos,
{
    pub skel: HostDriverSkel<P>,
    pub ctx: DriverCtx,
}

impl<P> HostDriver<P>
where
    P: Cosmos,
{
    pub fn new(skel: DriverSkel<P>, ctx: DriverCtx) -> Self {
        let skel = HostDriverSkel::new(skel);
        Self { skel, ctx }
    }
}

#[async_trait]
impl<P> Driver<P> for HostDriver<P>
where
    P: Cosmos,
{
    fn kind(&self) -> Kind {
        Kind::Host
    }
    async fn init(&mut self, skel: DriverSkel<P>, _ctx: DriverCtx) -> Result<(), P::Err> {
        skel.create_in_driver(
            PointSegTemplate::Exact("hosts".to_string()),
            Kind::Base.to_template(),
        )
        .await?;
        skel.logger
            .result(skel.status_tx.send(DriverStatus::Ready).await)
            .unwrap_or_default();
        Ok(())
    }

    async fn item(&self, point: &Point) -> Result<ItemSphere<P>, P::Err> {
        let host = self
            .skel
            .hosts
            .get(point)
            .ok_or(P::Err::not_found_msg(format!(
                "could not find host for :{}",
                point.to_string()
            )))?
            .value()
            .clone();
        let skel = HostItemSkel {
            skel: ItemSkel::new(point.clone(), Kind::Host, self.skel.skel.clone()),
            host,
        };
        Ok(ItemSphere::Handler(Box::new(HostItem::restore(
            skel,
            (),
            (),
        ))))
    }

    fn bind(&self) -> ArtRef<BindConfig> {
        HOST_DRIVER_BIND_CONFIG.clone()
    }

    async fn handler(&self) -> Box<dyn DriverHandler<P>> {
        Box::new(HostDriverHandler::restore(
            self.skel.clone(),
            self.ctx.clone(),
        ))
    }
}

#[derive(Clone)]
pub struct HostDriverPlatform<P>
where
    P: Cosmos,
{
    logger: RootLogger,
    phantom: PhantomData<P>,
}

impl<P> HostDriverPlatform<P>
where
    P: Cosmos,
{
    pub fn new(logger: RootLogger) -> Self {
        let phantom: PhantomData<P> = PhantomData::default();
        Self { logger, phantom }
    }
}

impl<P> HostPlatform for HostDriverPlatform<P>
where
    P: Cosmos,
{
    type Err = P::Err;

    fn root_logger(&self) -> RootLogger {
        self.logger.clone()
    }
}

#[derive(Clone)]
pub struct HostDriverSkel<P>
where
    P: Cosmos,
{
    pub skel: DriverSkel<P>,
    pub hosts: Arc<DashMap<Point, Arc<MechtronHost<HostDriverPlatform<P>>>>>,
    pub wasm_to_host_lookup: Arc<DashMap<Point, Point>>,
    pub factory: Arc<MechtronHostFactory<HostDriverPlatform<P>>>,
    pub hosts_base: Point,
}

impl<P> HostDriverSkel<P>
where
    P: Cosmos,
{
    pub fn new(skel: DriverSkel<P>) -> Self {
        let platform = HostDriverPlatform::new(skel.logger.logger.clone());
        let factory = Arc::new(MechtronHostFactory::new(platform));
        let hosts_base = skel.point.push("hosts").unwrap();
        Self {
            skel,
            hosts: Arc::new(DashMap::new()),
            wasm_to_host_lookup: Arc::new(DashMap::new()),
            factory,
            hosts_base,
        }
    }
}

pub struct HostDriverHandler<P>
where
    P: Cosmos,
{
    pub skel: HostDriverSkel<P>,
    pub ctx: DriverCtx,
}

impl<P> HostDriverHandler<P>
where
    P: Cosmos,
{
    fn restore(skel: HostDriverSkel<P>, ctx: DriverCtx) -> Self {
        HostDriverHandler { skel, ctx }
    }
}

impl<P> DriverHandler<P> for HostDriverHandler<P> where P: Cosmos {}

#[handler]
impl<P> HostDriverHandler<P>
where
    P: Cosmos,
{
    #[route("Hyp<Host>")]
    pub async fn host(&self, ctx: InCtx<'_, HyperSubstance>) -> Result<(), P::Err> {
        if let HyperSubstance::Host(host_cmd) = ctx.input {
            let config = host_cmd
                .details
                .properties
                .get("config")
                .ok_or("expected config property")
                .map_err(|e| SpaceErr::from_500(e))?;
            let config = Point::from_str(config.value.as_str())?;
            let config = self
                .skel
                .skel
                .skel
                .machine
                .artifacts
                .mechtron(&config)
                .await?;

            if !self.skel.wasm_to_host_lookup.contains_key(&config.wasm) {
                let mut properties = SetProperties::new();
                properties.push(PropertyMod::Set {
                    key: "wasm".to_string(),
                    value: config.wasm.clone().to_string(),
                    lock: false,
                });
                let create = Create {
                    template: Template {
                        point: PointTemplate {
                            parent: self.skel.hosts_base.clone(),
                            child_segment_template: PointSegTemplate::Pattern("host-%".to_string()),
                        },
                        kind: Kind::Host.to_template(),
                    },
                    properties,
                    strategy: Strategy::Commit,
                    state: StateSrc::None,
                };

                let mut create: DirectedProto = create.into();
                let pong = self.ctx.transmitter.ping(create).await?;
                pong.ok_or()?;

            }

            let host = self
                .skel
                .wasm_to_host_lookup
                .get(&config.wasm)
                .ok_or("expected Host to be in wasm_to_host_lookup")?
                .value()
                .clone();
            let host = self
                .skel
                .hosts
                .get(&host)
                .ok_or(P::Err::new(format!(
                    "expected host for point : {}",
                    host.to_string()
                )))?
                .value()
                .clone();

            host.create_mechtron(host_cmd.clone())?;

            self.skel
                .skel
                .registry()
                .assign_host(&host_cmd.details.stub.point, &host.details.stub.point)
                .await?;

            Ok(())
        } else {
            Err("expecting Host".into())
        }
    }

    #[route("Hyp<Assign>")]
    pub async fn assign(&self, ctx: InCtx<'_, HyperSubstance>) -> Result<(), P::Err> {
        if let HyperSubstance::Assign(assign) = ctx.input {

            let wasm = self.skel.skel.logger.result(
                assign
                    .details
                    .properties
                    .get(&"wasm".to_string())
                    .ok_or("wasm property must be set for a Mechtron Host"),
            )?;
            let wasm_point = Point::from_str(wasm.value.as_str())?;
            let wasm = self.skel.skel.artifacts().wasm(&wasm_point).await?;

            let bin = wasm.deref().deref().clone();
            let mechtron_host = Arc::new(
                self.skel
                    .factory
                    .create(assign.details.clone(), bin)
                    .map_err(|e| SpaceErr::from_500("host err"))?,
            );

            mechtron_host.create_guest()?;
            self.skel
                .hosts
                .insert(assign.details.stub.point.clone(), mechtron_host);
            self.skel
                .wasm_to_host_lookup
                .insert(wasm_point, assign.details.stub.point.clone());
            Ok(())
        } else {
            Err(P::Err::new("expected HyperSubstance<Assign>"))
        }
    }
}

#[derive(Clone)]
pub struct HostItemSkel<P, H>
where
    P: Cosmos,
    H: HostPlatform<Err = P::Err>,
{
    pub skel: ItemSkel<P>,
    pub host: Arc<MechtronHost<H>>,
}

pub struct HostItem<P, H>
where
    P: Cosmos,
    H: HostPlatform<Err = P::Err>,
{
    pub skel: HostItemSkel<P, H>,
}

impl<P, H> Item<P> for HostItem<P, H>
where
    P: Cosmos,
    H: HostPlatform<Err = P::Err>,
{
    type Skel = HostItemSkel<P, H>;
    type Ctx = ();
    type State = ();

    fn restore(skel: Self::Skel, ctx: Self::Ctx, state: Self::State) -> Self {
        Self { skel }
    }
}

#[handler]
impl<P, H> HostItem<P, H>
where
    P: Cosmos,
    H: HostPlatform<Err = P::Err>,
{
    #[route("Hyp<Transport>")]
    async fn transport(&self, ctx: InCtx<'_, UltraWave>) {
        if let Ok(Some(wave)) = self.skel.host.route(ctx.input.clone()) {
            let re = wave.clone().to_reflected().unwrap();
            ctx.transmitter.route(wave).await;
        }
    }
}

#[async_trait]
impl<P, H> ItemHandler<P> for HostItem<P, H>
where
    P: Cosmos,
    H: HostPlatform<Err = P::Err>,
{
    async fn bind(&self) -> Result<ArtRef<BindConfig>, P::Err> {
        Ok(HOST_BIND_CONFIG.clone())
    }
}

pub struct MechtronDriverFactory {}

impl MechtronDriverFactory {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl<P> HyperDriverFactory<P> for MechtronDriverFactory
where
    P: Cosmos,
{
    fn kind(&self) -> KindSelector {
        KindSelector::from_base(BaseKind::Mechtron)
    }

    async fn create(
        &self,
        skel: HyperStarSkel<P>,
        driver_skel: DriverSkel<P>,
        ctx: DriverCtx,
    ) -> Result<Box<dyn Driver<P>>, P::Err> {
        Ok(Box::new(MechtronDriver::new(driver_skel, ctx)))
    }
}

pub struct MechtronDriver<P>
where
    P: Cosmos,
{
    pub ctx: DriverCtx,
    pub skel: DriverSkel<P>,
}

#[async_trait]
impl<P> Driver<P> for MechtronDriver<P>
where
    P: Cosmos,
{
    fn kind(&self) -> Kind {
        Kind::Mechtron
    }

    async fn item(&self, point: &Point) -> Result<ItemSphere<P>, P::Err> {
        let ctx = self.skel.item_ctx(point, Layer::Core)?;
        let skel = ItemSkel::new(point.clone(), Kind::Mechtron, self.skel.clone());
        let mechtron = Mechtron::restore(skel, ctx, ());
        Ok(ItemSphere::Router(Box::new(mechtron)))
    }
    async fn handler(&self) -> Box<dyn DriverHandler<P>> {
        Box::new(MechtronDriverHandler::restore(
            self.skel.clone(),
            self.ctx.clone(),
        ))
    }
}

impl<P> MechtronDriver<P>
where
    P: Cosmos,
{
    pub fn new(skel: DriverSkel<P>, ctx: DriverCtx) -> Self {
        Self { skel, ctx }
    }
}

pub struct MechtronDriverHandler<P>
where
    P: Cosmos,
{
    skel: DriverSkel<P>,
    ctx: DriverCtx,
}

impl<P> MechtronDriverHandler<P>
where
    P: Cosmos,
{
    fn restore(skel: DriverSkel<P>, ctx: DriverCtx) -> Self {
        MechtronDriverHandler { skel, ctx }
    }
}

impl<P> DriverHandler<P> for MechtronDriverHandler<P> where P: Cosmos {}

#[handler]
impl<P> MechtronDriverHandler<P>
where
    P: Cosmos,
{
    #[route("Hyp<Assign>")]
    async fn assign(&self, ctx: InCtx<'_, HyperSubstance>) -> Result<(), P::Err> {
        if let HyperSubstance::Assign(assign) = ctx.input {
            let logger = self.skel.logger.push_mark("assign")?;

            let config = assign
                .details
                .properties
                .get(&"config".to_string())
                .ok_or("config property must be set for a Mechtron")?;

            let config = Point::from_str(config.value.as_str())?;
            let config = self
                .skel
                .logger
                .result(self.skel.artifacts().mechtron(&config).await)?;
            let config = config.contents();

            let host = self.skel.drivers().local_driver_lookup(Kind::Host).await?.ok_or(P::Err::new("missing Host Driver which must be on the same Star as the Mechtron Driver in order for it to work"))?;
            let mut wave = DirectedProto::ping();
            wave.method(HypMethod::Host);
            wave.to(host.to_surface().with_layer(Layer::Core));
            wave.body(HyperSubstance::Host(assign.clone().to_host_cmd(config)).into());
            let pong = self.ctx.transmitter.ping(wave).await?;
            pong.ok_or()?;
            Ok(())
        } else {
            Err(P::Err::new("MechtronDriverHandler expecting Assign"))
        }
    }
}

pub struct Mechtron<P>
where
    P: Cosmos,
{
    skel: ItemSkel<P>,
    ctx: ItemCtx,
}

impl<P> Item<P> for Mechtron<P>
where
    P: Cosmos,
{
    type Skel = ItemSkel<P>;
    type Ctx = ItemCtx;
    type State = ();

    fn restore(skel: Self::Skel, ctx: Self::Ctx, _state: Self::State) -> Self {
        Self { skel, ctx }
    }
}

#[async_trait]
impl<P> TraversalRouter for Mechtron<P>
where
    P: Cosmos,
{
    async fn traverse(&self, traversal: Traversal<UltraWave>) -> Result<(), SpaceErr> {
        let wave = traversal.payload;
        let record = self
            .skel
            .skel
            .registry()
            .record(&self.skel.point)
            .await
            .map_err(|e| e.to_space_err())?;
        let location = record.location;

        let host = location
            .host
            .ok_or::<SpaceErr>("expected Mechtron to have an assigned Host".into())?
            .to_surface()
            .with_layer(Layer::Core);

        let transport =
            wave.wrap_in_transport(self.skel.point.to_surface().with_layer(Layer::Core), host);
        self.ctx.transmitter.signal(transport).await?;
        Ok(())
    }
}

#[async_trait]
impl<P> ItemRouter<P> for Mechtron<P>
where
    P: Cosmos,
{
    async fn bind(&self) -> Result<ArtRef<BindConfig>, P::Err> {
        Ok(MECHTRON_BIND_CONFIG.clone())
    }
}