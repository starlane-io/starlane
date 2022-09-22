use std::ops::Deref;
use crate::driver::{
    Driver, DriverAvail, DriverCtx, DriverSkel, HyperDriverFactory, ItemHandler, ItemSphere,
};
use crate::star::HyperStarSkel;
use crate::Hyperverse;
use cosmic_universe::artifact::ArtRef;
use cosmic_universe::config::bind::BindConfig;
use cosmic_universe::kind::{BaseKind, Kind};
use cosmic_universe::loc::{Layer, Point, ToSurface};
use cosmic_universe::parse::bind_config;
use cosmic_universe::selector::KindSelector;
use cosmic_universe::util::log;
use std::str::FromStr;
use std::sync::Arc;
use dashmap::DashMap;
use cosmic_universe::err::UniErr;
use cosmic_universe::hyper::{Assign, HyperSubstance};
use cosmic_universe::wave::{DirectedProto, DirectedWave, Pong, Wave};
use cosmic_universe::wave::core::hyp::HypMethod;
use cosmic_universe::wave::exchange::InCtx;
use mechtron_host::{MechtronHost, MechtronHostFactory};

lazy_static! {
    static ref MECHTRON_BIND_CONFIG: ArtRef<BindConfig> = ArtRef::new(
        Arc::new(mechtron_bind()),
        Point::from_str("GLOBAL::repo:1.0.0:/bind/mechtron.bind").unwrap()
    );
}

fn mechtron_bind() -> BindConfig {
    log(bind_config(
        r#"
    Bind(version=1.0.0)
    {
    }
    "#,
    ))
    .unwrap()
}

pub struct HostDriverFactory {
    pub avail: DriverAvail,
}

impl HostDriverFactory {
    pub fn new(avail: DriverAvail) -> Self {
        Self { avail }
    }
}

#[async_trait]
impl<P> HyperDriverFactory<P> for HostDriverFactory
where
    P: Hyperverse,
{
    fn kind(&self) -> KindSelector {
        KindSelector::from_base(BaseKind::Host )
    }

    async fn create(
        &self,
        skel: HyperStarSkel<P>,
        driver_skel: DriverSkel<P>,
        ctx: DriverCtx,
    ) -> Result<Box<dyn Driver<P>>, P::Err> {
        Ok(Box::new(HostDriver::new(self.avail.clone())))
    }
}

pub struct HostDriver<P> where P: Hyperverse {
    pub skel: DriverSkel<P>,
    pub hosts: DashMap<Point,Arc<MechtronHost>>,
    pub wasm_to_host_lookup: DashMap<Point,Point>,
    pub factory: MechtronHostFactory
}

#[handler]
impl <P> HostDriver<P> where P: Hyperverse {
    pub fn new(skel: DriverSkel<P>) -> Self {
        let hosts = DashMap::new();
        let factory = MechtronHostFactory::new();
        let wasm_to_host_lookup = DashMap::new();
        Self {  skel, hosts, factory, wasm_to_host_lookup }
    }

    #[route("Hyp<Host>")]
    pub async fn host( &self, ctx: InCtx<'_,HyperSubstance>) -> Result<(),UniErr>
    {
        if let HyperSubstance::Host(host) = ctx.input {
            let config = assign.details.properties.get("config").ok_or("expected config property")?;
            let config = Point::from_str(config.value.as_str())?;
            let config = self.skel.skel.machine.artifacts.mechtron(&config).await?;

            if !self.wasm_to_host_lookup.contains_key(&config.bin) {
                let wasm = self.skel.skel.machine.artifacts.wasm(&config.bin).await?;
                let bin = wasm.deref().deref().clone();
                let host = Arc::new(self.factory.create(assign.details.stub.point.clone(), bin)?);
                self.hosts.insert(assign.details.stub.point.clone(), host);
                self.wasm_to_host_lookup.insert(config.bin.clone(), assign.details.stub.point.clone())
            }
            Ok(())
        } else {
            Err("expecting Host".into())
        }
    }
}

#[async_trait]
impl<P> Driver<P> for HostDriver<P>
where
    P: Hyperverse,
{
    fn kind(&self) -> Kind {
        Kind::Host
    }

    async fn item(&self, point: &Point) -> Result<ItemSphere<P>, P::Err> {
        Ok(ItemSphere::Handler(Box::new(HostItem)))
    }
}

pub struct HostItem;

#[handler]
impl HostItem {}

#[async_trait]
impl<P> ItemHandler<P> for HostItem
where
    P: Hyperverse,
{
    async fn bind(&self) -> Result<ArtRef<BindConfig>, P::Err> {
        Ok(MECHTRON_BIND_CONFIG.clone())
    }
}





pub struct MechtronDriverFactory {
    pub avail: DriverAvail,
}

impl MechtronDriverFactory {
    pub fn new(avail: DriverAvail) -> Self {
        Self { avail }
    }
}

#[async_trait]
impl<P> HyperDriverFactory<P> for MechtronDriverFactory
    where
        P: Hyperverse,
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

pub struct MechtronDriver<P> where P: Hyperverse {
    pub ctx: DriverCtx,
    pub skel: DriverSkel<P>
}

#[handler]
impl <P> MechtronDriver<P> where P: Hyperverse {
    pub fn new(skel: DriverSkel<P>, ctx: DriverCtx) -> Self {
        Self {  skel, ctx }
    }
}

#[async_trait]
impl<P> Driver<P> for MechtronDriver<P>
    where
        P: Hyperverse,
{
    fn kind(&self) -> Kind {
        Kind::Mechtron
    }

    async fn item(&self, point: &Point) -> Result<ItemSphere<P>, P::Err> {
        Ok(ItemSphere::Handler(Box::new(Mechtron)))
    }

    async fn assign(&self, assign: Assign) -> Result<(), P::Err> {
        let host = self.skel.local_driver_lookup(Kind::Host).await?.ok_or("cannot find local Host Driver".into())?;
        let mut wave = DirectedProto::ping();
        wave.method(HypMethod::Host);
        wave.to(host.to_surface().with_layer(Layer::Core));
        wave.body(assign.to_host().into());
        let pong: Wave<Pong> = self.ctx.transmitter.direct(wave).await?;
        pong.ok_or()?;
        Ok(())
    }
}

pub struct Mechtron;

#[handler]
impl Mechtron {}

#[async_trait]
impl<P> ItemHandler<P> for Mechtron
    where
        P: Hyperverse,
{
    async fn bind(&self) -> Result<ArtRef<BindConfig>, P::Err> {
        Ok(MECHTRON_BIND_CONFIG.clone())
    }
}
