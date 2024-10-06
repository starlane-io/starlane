use crate::driver::{Driver, DriverCtx, DriverErr, DriverSkel, HyperDriverFactory, ParticleHandler, ParticleSphere};
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
use std::str::FromStr;
use std::sync::Arc;

static SPACE_BIND_CONFIG: Lazy<ArtRef<BindConfig>> = Lazy::new(|| {
    ArtRef::new(
        Arc::new(space_bind()),
        Point::from_str("GLOBAL::repo:1.0.0:/bind/space.bind").unwrap(),
    )
});

fn space_bind() -> BindConfig {
    log(bind_config(
        r#"
    Bind(version=1.0.0)
    {
    }
    "#,
    ))
    .unwrap()
}

pub struct SpaceDriverFactory;

impl SpaceDriverFactory {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl HyperDriverFactory for SpaceDriverFactory
{
    fn kind(&self) -> Kind {
        Kind::Space
    }

    fn selector(&self) -> KindSelector {
        KindSelector::from_base(BaseKind::Space)
    }

    async fn create(
        &self,
        skel: HyperStarSkel,
        driver_skel: DriverSkel,
        ctx: DriverCtx,
    ) -> Result<Box<dyn Driver>, DriverErr> {
        Ok(Box::new(SpaceDriver))
    }
}

pub struct SpaceDriver;

#[async_trait]
impl Driver for SpaceDriver
{
    fn kind(&self) -> Kind {
        Kind::Space
    }

    async fn particle(&self, point: &Point) -> Result<ParticleSphere, DriverErr> {
        Ok(ParticleSphere::Handler(Box::new(Space)))
    }
}

pub struct Space;

#[handler]
impl Space {}

#[async_trait]
impl ParticleHandler for Space
{
    async fn bind(&self) -> Result<ArtRef<BindConfig>, DriverErr> {
        Ok(SPACE_BIND_CONFIG.clone())
    }
}
