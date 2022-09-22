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

lazy_static! {
    static ref HOST_BIND_CONFIG: ArtRef<BindConfig> = ArtRef::new(
        Arc::new(host_bind()),
        Point::from_str("GLOBAL::repo:1.0.0:/bind/host.bind").unwrap()
    );
}

fn host_bind() -> BindConfig {
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
        KindSelector::from_base(BaseKind::Host)
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

pub struct HostDriver {
    pub avail: DriverAvail,
}

#[handler]
impl HostDriver {
    pub fn new(avail: DriverAvail) -> Self {
        Self { avail }
    }
}

#[async_trait]
impl<P> Driver<P> for HostDriver
where
    P: Hyperverse,
{
    fn kind(&self) -> Kind {
        Kind::Host
    }

    async fn item(&self, point: &Point) -> Result<ItemSphere<P>, P::Err> {
        Ok(ItemSphere::Handler(Box::new(Host)))
    }
}

pub struct Host;

#[handler]
impl Host {}

#[async_trait]
impl<P> ItemHandler<P> for Host
where
    P: Hyperverse,
{
    async fn bind(&self) -> Result<ArtRef<BindConfig>, P::Err> {
        Ok(HOST_BIND_CONFIG.clone())
    }
}
