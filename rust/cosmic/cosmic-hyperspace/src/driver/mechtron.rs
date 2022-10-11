use crate::driver::{Driver, DriverAvail, DriverCtx, DriverHandler, DriverSkel, HyperDriverFactory, Item, ItemCtx, ItemHandler, ItemRouter, ItemSkel, ItemSphere};
use crate::star::{HyperStarSkel, LayerInjectionRouter};
use crate::Cosmos;
use cosmic_space::artifact::ArtRef;
use cosmic_space::config::bind::BindConfig;
use cosmic_space::err::SpaceErr;
use cosmic_space::hyper::{Assign, HyperSubstance};
use cosmic_space::kind::{BaseKind, Kind};
use cosmic_space::loc::{Layer, Point, ToSurface};
use cosmic_space::log::RootLogger;
use cosmic_space::parse::bind_config;
use cosmic_space::selector::KindSelector;
use cosmic_space::util::log;
use cosmic_space::wave::core::hyp::HypMethod;
use cosmic_space::wave::exchange::asynch::{InCtx, TraversalRouter};
use cosmic_space::wave::{DirectedProto, DirectedWave, Pong, UltraWave, Wave};
use dashmap::DashMap;
use mechtron_host::{HostPlatform, MechtronHost, MechtronHostFactory};
use std::marker::PhantomData;
use std::ops::Deref;
use std::str::FromStr;
use std::sync::Arc;
use cosmic_space::particle::traversal::{Traversal, TraversalDirection};
use crate::err::HyperErr;

lazy_static! {
    static ref HOST_BIND_CONFIG: ArtRef<BindConfig> = ArtRef::new(
        Arc::new(host_bind()),
        Point::from_str("GLOBAL::repo:1.0.0:/bind/host.bind").unwrap()
    );
    static ref MECHTRON_BIND_CONFIG: ArtRef<BindConfig> = ArtRef::new(
        Arc::new(mechtron_bind()),
        Point::from_str("GLOBAL::repo:1.0.0:/bind/mechtron.bind").unwrap()
    );
}

fn host_bind() -> BindConfig {
    log(bind_config(
        r#"
    Bind(version=1.0.0)
    {
       Route<Hyp<Host>> -> (()) => &;
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

    async fn item(&self, point: &Point) -> Result<ItemSphere<P>, P::Err> {
        Ok(ItemSphere::Handler(Box::new(HostItem)))
    }

    fn bind(&self) -> ArtRef<BindConfig> {
        HOST_BIND_CONFIG.clone()
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
}

impl<P> HostDriverSkel<P>
where
    P: Cosmos,
{
    pub fn new(skel: DriverSkel<P>) -> Self {
        let platform = HostDriverPlatform::new(skel.logger.logger.clone());
        let factory = Arc::new(MechtronHostFactory::new(platform));
        Self {
            skel,
            hosts: Arc::new(DashMap::new()),
            wasm_to_host_lookup: Arc::new(DashMap::new()),
            factory,
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
        println!("HOST DRIVER RECEIVED REQUEST!");
        if let HyperSubstance::Host(host) = ctx.input {
            let config = host
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

            if !self.skel.wasm_to_host_lookup.contains_key(&config.bin) {
                let wasm = self
                    .skel
                    .skel
                    .skel
                    .machine
                    .artifacts
                    .wasm(&config.bin)
                    .await?;
                let bin = wasm.deref().deref().clone();
                let mechtron_host = Arc::new(
                    self.skel
                        .factory
                        .create(host.details.clone(), bin)
                        .map_err(|e| SpaceErr::from_500("host err"))?,
                );

                mechtron_host.init( host.details.clone() )?;

                self.skel
                    .hosts
                    .insert(host.details.stub.point.clone(), mechtron_host);
                self.skel
                    .wasm_to_host_lookup
                    .insert(config.bin.clone(), host.details.stub.point.clone());
            }
            Ok(())
        } else {
            Err("expecting Host".into())
        }
    }
}

pub struct HostItem;

#[handler]
impl HostItem {}

#[async_trait]
impl<P> ItemHandler<P> for HostItem
where
    P: Cosmos,
{
    async fn bind(&self) -> Result<ArtRef<BindConfig>, P::Err> {
        Ok(MECHTRON_BIND_CONFIG.clone())
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
        let ctx = self.skel.item_ctx(point,Layer::Core)?;
        let skel = ItemSkel::new( point.clone(), Kind::Mechtron, self.skel.clone() );
        let mechtron = Mechtron::restore(skel,ctx,());
        Ok(ItemSphere::Router(Box::new( mechtron )))
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
            println!("\tASSIGNING MECHTRON!");
            let logger = self.skel.logger.push_mark("assign")?;

            let config = assign
                .details
                .properties
                .get(&"config".to_string() )
                .ok_or("config property must be set for a Mechtron")?;

            let config = Point::from_str(config.value.as_str())?;
            let config = self.skel.logger.result(self.skel.artifacts().mechtron(&config).await)?;
            let config = config.contents();

            let host = self.skel.drivers().local_driver_lookup(Kind::Host).await?.ok_or(P::Err::new("missing Host Driver which must be on the same Star as the Mechtron Driver in order for it to work"))?;
            let mut wave = DirectedProto::ping();
            wave.method(HypMethod::Host);
            println!("\tSending HOST command to {}", host.to_string());
            wave.to(host.to_surface().with_layer(Layer::Core));
            wave.body(HyperSubstance::Host(assign.clone().to_host_cmd(config)).into());
            let pong = self.ctx.transmitter.ping(wave).await?;
            pong.ok_or()?;
            Ok(())
        } else {
            Err(P::Err::new(
                "MechtronDriverHandler expecting Assign",
            ))
        }
    }
}

pub struct Mechtron<P> where P: Cosmos {
    skel: ItemSkel<P>,
    ctx: ItemCtx
}

impl <P> Item<P> for Mechtron<P> where P: Cosmos {
    type Skel = ItemSkel<P>;
    type Ctx = ItemCtx;
    type State = ();

    fn restore(skel: Self::Skel, ctx: Self::Ctx, _state: Self::State) -> Self {
        Self {
            skel,
            ctx
        }
    }
}


#[async_trait]
impl<P> TraversalRouter for Mechtron<P> where P: Cosmos {
    async fn traverse(&self, traversal: Traversal<UltraWave>) -> Result<(), SpaceErr> {
        let wave = traversal.payload;
        let record = self.skel.skel.registry().record( &self.skel.point ).await.map_err(|e|e.to_space_err())?;
        let location = record.location.ok_or::<SpaceErr>("expected Mechtron to have an assigned Host".into() )?;
        let host = location.host.ok_or::<SpaceErr>("expected Mechtron to have an assigned Host".into() )?.to_surface().with_layer(Layer::Core );

        let transport = wave.wrap_in_transport(self.skel.point.to_surface().with_layer(Layer::Core),host );
        self.ctx.transmitter.signal(transport).await?;
        Ok(())
    }
}

#[async_trait]
impl <P> ItemRouter<P> for Mechtron<P> where P:Cosmos {
    async fn bind(&self) -> Result<ArtRef<BindConfig>, P::Err> {
        Ok(MECHTRON_BIND_CONFIG.clone())
    }
}

