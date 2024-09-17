use crate::hyper::space::driver::{Driver, DriverAvail, DriverCtx, DriverSkel, HyperDriverFactory, ItemHandler, ItemSphere, DRIVER_BIND};
use crate::hyper::space::star::HyperStarSkel;
use crate::hyper::space::Cosmos;
use once_cell::sync::Lazy;
use starlane_space::artifact::ArtRef;
use starlane_space::config::bind::BindConfig;
use starlane_space::kind::{BaseKind, Kind};
use starlane_space::parse::bind_config;
use starlane_space::point::Point;
use starlane_space::selector::KindSelector;
use starlane_space::util::log;
use std::str::FromStr;
use std::sync::Arc;
use starlane_space::wave::core::CoreBounce;
use starlane_space::wave::exchange::asynch::{DirectedHandler, RootInCtx};

static BASE_BIND_CONFIG: Lazy<ArtRef<BindConfig>> = Lazy::new(|| {
    ArtRef::new(
        Arc::new(base_bind()),
        Point::from_str("GLOBAL::repo:1.0.0:/bind/base.bind").unwrap(),
    )
});

fn base_bind() -> BindConfig {
    log(bind_config(
        r#"
    Bind(version=1.0.0)
    {
    }
    "#,
    ))
    .unwrap()
}

pub struct BaseDriverFactory {
    pub avail: DriverAvail,
}

impl BaseDriverFactory {
    pub fn new(avail: DriverAvail) -> Self {
        Self { avail }
    }
}

#[async_trait]
impl<P> HyperDriverFactory<P> for BaseDriverFactory
where
    P: Cosmos,
{
    fn kind(&self) -> KindSelector {
        KindSelector::from_base(BaseKind::Base)
    }

    async fn create(
        &self,
        skel: HyperStarSkel<P>,
        driver_skel: DriverSkel<P>,
        ctx: DriverCtx,
    ) -> Result<Box<dyn Driver<P>>, P::Err> {
        Ok(Box::new(BaseDriver::new(self.avail.clone())))
    }
}

pub struct BaseDriver {
    pub avail: DriverAvail,
}

impl BaseDriver {
    pub fn new(avail: DriverAvail) -> Self {
        Self { avail }
    }
}

#[async_trait]
impl<P> Driver<P> for BaseDriver
where
    P: Cosmos,
{
    fn kind(&self) -> Kind {
        Kind::Base
    }

    async fn item(&self, point: &Point) -> Result<ItemSphere<P>, P::Err> {
        println!("ITEM get BASE");
        Ok(ItemSphere::Handler(Box::new(Base)))
    }
}

pub struct Base;

#[handler]
impl Base {}


#[async_trait]
impl<P> ItemHandler<P> for Base
where
    P: Cosmos,
{
    async fn bind(&self) -> Result<ArtRef<BindConfig>, P::Err> {
        Ok(DRIVER_BIND.clone())
    }
}
