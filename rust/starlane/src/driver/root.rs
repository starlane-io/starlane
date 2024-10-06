use crate::driver::{Driver, DriverCtx, DriverErr, DriverSkel, HyperDriverFactory, Particle, ParticleHandler, ParticleSphere};
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
use std::marker::PhantomData;
use std::str::FromStr;
use std::sync::Arc;

static ROOT_BIND_CONFIG: Lazy<ArtRef<BindConfig>> = Lazy::new(|| {
    ArtRef::new(
        Arc::new(root_bind()),
        Point::from_str("GLOBAL::repo:1.0.0:/bind/root.bind").unwrap(),
    )
});

fn root_bind() -> BindConfig {
    log(bind_config(
        r#"
    Bind(version=1.0.0)
    { }
    "#,
    ))
    .unwrap()
}

pub struct RootDriverFactory;

impl RootDriverFactory {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl HyperDriverFactory for RootDriverFactory

{
    fn kind(&self) -> Kind {
        Kind::Root
    }

    fn selector(&self) -> KindSelector {
        KindSelector::from_base(BaseKind::Root)
    }

    async fn create(
        &self,
        skel: HyperStarSkel,
        driver_skel: DriverSkel,
        ctx: DriverCtx,
    ) -> Result<Box<dyn Driver>, DriverErr> {
        Ok(Box::new(RootDriver {}))
    }
}

pub struct RootDriver;

#[async_trait]
impl Driver for RootDriver

{
    fn kind(&self) -> Kind {
        Kind::Root
    }

    async fn particle(&self, point: &Point) -> Result<ParticleSphere, DriverErr> {
        Ok(ParticleSphere::Handler(Box::new(Root::restore((), (), ()))))
    }
}

pub struct Root

{
}

impl Root

{
    pub fn new() -> Self {
        Self {
        }
    }
}

impl Particle for Root

{
    type Skel = ();
    type Ctx = ();
    type State = ();

    fn restore(skel: Self::Skel, ctx: Self::Ctx, state: Self::State) -> Self {
        Self::new()
    }
}

#[handler]
impl Root {}

#[async_trait]
impl ParticleHandler for Root

{
    async fn bind(&self) -> Result<ArtRef<BindConfig>, DriverErr> {
        Ok(ROOT_BIND_CONFIG.clone())
    }
}
