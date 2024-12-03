use crate::hyperspace::driver::{
    Driver, DriverCtx, DriverErr, DriverSkel, HyperDriverFactory, Particle, ParticleSphere,
    ParticleSphereInner, StdParticleErr,
};
use crate::hyperspace::platform::Platform;
use crate::hyperspace::star::HyperStarSkel;
use crate::space::artifact::ArtRef;
use crate::space::config::bind::BindConfig;
use crate::space::kind::{BaseKind, Kind};
use crate::space::parse::bind_config;
use crate::space::point::Point;
use crate::space::selector::KindSelector;
use crate::space::util::log;
use crate::space::wave::core::CoreBounce;
use crate::space::wave::exchange::asynch::{DirectedHandler, RootInCtx};
use async_trait::async_trait;
use once_cell::sync::Lazy;
use starlane_macros::{handler, DirectedHandler};
use std::str::FromStr;
use std::sync::Arc;

pub struct SpaceDriverFactory;

impl SpaceDriverFactory {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl HyperDriverFactory for SpaceDriverFactory {
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
impl Driver for SpaceDriver {
    fn kind(&self) -> Kind {
        Kind::Space
    }

    async fn particle(&self, point: &Point) -> Result<ParticleSphere, DriverErr> {
        let space = Space::restore((), (), ());
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
