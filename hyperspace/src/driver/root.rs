use crate::driver::{
    Driver, DriverCtx, DriverErr, DriverSkel, HyperDriverFactory, Particle, ParticleSphere
    , StdParticleErr,
};

use crate::platform::Platform;
use crate::star::HyperStarSkel;
use space::kind::{BaseKind, Kind};
use space::point::Point;
use space::selector::KindSelector;
use space::wave::exchange::asynch::DirectedHandler;
use async_trait::async_trait;
use starlane_macros::{handler, DirectedHandler};
use std::str::FromStr;

pub struct RootDriverFactory;

impl RootDriverFactory {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl HyperDriverFactory for RootDriverFactory {
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
impl Driver for RootDriver {
    fn kind(&self) -> Kind {
        Kind::Root
    }

    async fn particle(&self, point: &Point) -> Result<ParticleSphere, DriverErr> {
        let root = Root::restore((), (), ());
        Ok(root.sphere()?)
    }
}

#[derive(DirectedHandler)]
pub struct Root {}

impl Root {
    pub fn new() -> Self {
        Self {}
    }
}

impl Particle for Root {
    type Skel = ();
    type Ctx = ();
    type State = ();
    type Err = StdParticleErr;

    fn restore(_: Self::Skel, _: Self::Ctx, _: Self::State) -> Self {
        Self::new()
    }

    fn sphere(self) -> Result<ParticleSphere, Self::Err> {
        Ok(ParticleSphere::new_handler(self))
    }
}

#[handler]
impl Root {}
