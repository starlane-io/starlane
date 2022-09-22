use crate::driver::{
    Driver, DriverAvail, DriverCtx, DriverSkel, HyperDriverFactory, ItemHandler, ItemSphere,
};
use crate::star::HyperStarSkel;
use crate::Hyperverse;
use cosmic_universe::artifact::ArtRef;
use cosmic_universe::config::bind::BindConfig;
use cosmic_universe::kind::{BaseKind, Kind};
use cosmic_universe::loc::Point;
use cosmic_universe::parse::bind_config;
use cosmic_universe::selector::KindSelector;
use cosmic_universe::util::log;
use std::str::FromStr;
use std::sync::Arc;
use dashmap::DashMap;
use cosmic_universe::hyper::Assign;

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

pub struct MechtronHostDriverFactory {
    pub avail: DriverAvail,
}

impl MechtronHostDriverFactory {
    pub fn new(avail: DriverAvail) -> Self {
        Self { avail }
    }
}

#[async_trait]
impl<P> HyperDriverFactory<P> for MechtronHostDriverFactory
where
    P: Hyperverse,
{
    fn kind(&self) -> KindSelector {
        KindSelector::from_base(BaseKind::MechtronHost)
    }

    async fn create(
        &self,
        skel: HyperStarSkel<P>,
        driver_skel: DriverSkel<P>,
        ctx: DriverCtx,
    ) -> Result<Box<dyn Driver<P>>, P::Err> {
        Ok(Box::new(MechtronHostDriver::new(self.avail.clone())))
    }
}

pub struct MechtronHostDriver {
    pub avail: DriverAvail,
    pub hosts: DashMap<Point,MechtronHostHost>
}

#[handler]
impl MechtronHostDriver {
    pub fn new(avail: DriverAvail) -> Self {
        let hosts = DashMap::new();
        Self { avail, hosts }
    }
}

#[async_trait]
impl<P> Driver<P> for MechtronHostDriver
where
    P: Hyperverse,
{
    fn kind(&self) -> Kind {
        Kind::MechtronHost
    }

    async fn item(&self, point: &Point) -> Result<ItemSphere<P>, P::Err> {
        Ok(ItemSphere::Handler(Box::new(MechtronHost)))
    }

    async fn assign(&self, assign: Assign) -> Result<(), P::Err> {
        assign.details.properties.get("config")
    }
}

pub struct MechtronHost;

#[handler]
impl MechtronHost {}

#[async_trait]
impl<P> ItemHandler<P> for MechtronHost
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
        Ok(Box::new(MechtronDriver::new(self.avail.clone())))
    }
}

pub struct MechtronDriver {
    pub avail: DriverAvail,
    pub hosts: DashMap<Point,MechtronHost>
}

#[handler]
impl MechtronDriver {
    pub fn new(avail: DriverAvail) -> Self {
        let hosts = DashMap::new();
        Self { avail, hosts }
    }
}

#[async_trait]
impl<P> Driver<P> for MechtronDriver
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
        // when a mechtron is assigned first we  have to find it's host

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
