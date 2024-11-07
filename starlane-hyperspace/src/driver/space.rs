use crate::driver::{Driver, DriverCtx, DriverErr, DriverSkel, HyperDriverFactory, Particle, ParticleSphere, ParticleSphereInner, StdParticleErr};
use crate::platform::Platform;
use crate::star::HyperStarSkel;
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
use starlane::space::wave::core::CoreBounce;
use starlane::space::wave::exchange::asynch::{DirectedHandler, RootInCtx};


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
        let space = Space::restore((),(),());
        Ok(space.sphere()?)
    }
}

#[derive(DirectedHandler)]
pub struct Space;

#[handler]
impl Space {}

impl Particle for Space {
    type Skel = ();
    type Ctx = ();
    type State = ();
    type Err = StdParticleErr;

    fn restore(_: Self::Skel, _: Self::Ctx, _: Self::State) -> Self {
        Space
    }


    fn sphere(self) -> Result<ParticleSphere, Self::Err> {
       Ok(ParticleSphere::new_handler(self))
    }
}


