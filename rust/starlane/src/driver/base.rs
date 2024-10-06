use crate::driver::{
    Driver, DriverAvail, DriverCtx, DriverSkel, HyperDriverFactory, ParticleHandler, ParticleSphere,
    DRIVER_BIND,
};

pub use starlane_space as starlane;

use crate::platform::Platform;
use crate::hyperspace::star::HyperStarSkel;
use once_cell::sync::Lazy;
use starlane::space::artifact::ArtRef;
use starlane::space::config::bind::BindConfig;
use starlane::space::kind::{BaseKind, Kind};
use starlane::space::parse::bind_config;
use starlane::space::point::Point;
use starlane::space::selector::KindSelector;
use starlane::space::util::log;
use starlane::space::wave::exchange::asynch::DirectedHandler;
use std::str::FromStr;
use std::sync::Arc;

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
impl HyperDriverFactory for BaseDriverFactory

{
    fn kind(&self) -> Kind {
        Kind::Base
    }

    fn selector(&self) -> KindSelector {
        KindSelector::from_base(BaseKind::Base)
    }

    async fn create(
        &self,
        skel: HyperStarSkel,
        driver_skel: DriverSkel,
        ctx: DriverCtx,
    ) -> Result<Box<dyn Driver>, P::Err> {
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
impl Driver for BaseDriver

{
    fn kind(&self) -> Kind {
        Kind::Base
    }

    async fn particle(&self, point: &Point) -> Result<ParticleSphere, P::Err> {
        println!("ITEM get BASE");
        Ok(ParticleSphere::Handler(Box::new(Base)))
    }
}

pub struct Base;

#[handler]
impl Base {}

#[async_trait]
impl ParticleHandler for Base
{
    async fn bind(&self) -> Result<ArtRef<BindConfig>, P::Err> {
        Ok(DRIVER_BIND.clone())
    }
}
