use crate::driver::{
    Driver, DriverAvail, DriverCtx, DriverErr, DriverSkel, HyperDriverFactory, Particle,
    ParticleSphere, StdParticleErr,
};

use crate::platform::Platform;
use crate::star::HyperStarSkel;
use async_trait::async_trait;
use space::kind::{BaseKind, Kind};
use space::point::Point;
use space::selector::KindSelector;
use space::wave::exchange::asynch::DirectedHandler;
use starlane_macros::{handler, DirectedHandler};
use std::str::FromStr;

pub struct BaseDriverFactory {
    pub avail: DriverAvail,
}

impl BaseDriverFactory {
    pub fn new(avail: DriverAvail) -> Self {
        Self { avail }
    }
}

#[async_trait]
impl HyperDriverFactory for BaseDriverFactory {
    fn kind(&self) -> Kind {
        Kind::Base
    }

    fn selector(&self) -> KindSelector {
        KindSelector::from_base(BaseKind::Base)
    }

    async fn create(
        &self,
        _: HyperStarSkel,
        _: DriverSkel,
        _: DriverCtx,
    ) -> Result<Box<dyn Driver>, DriverErr> {
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
impl Driver for BaseDriver {
    fn kind(&self) -> Kind {
        Kind::Base
    }

    async fn particle(&self, point: &Point) -> Result<ParticleSphere, DriverErr> {
        let base = Base::restore((), (), ());

        Ok(base.sphere()?)
    }
}

#[derive(DirectedHandler)]
pub struct Base;

impl Particle for Base {
    type Skel = ();
    type Ctx = ();
    type State = ();
    type Err = StdParticleErr;

    fn restore(_: Self::Skel, _: Self::Ctx, _: Self::State) -> Self {
        Base
    }

    fn sphere(self) -> Result<ParticleSphere, Self::Err> {
        Ok(ParticleSphere::new_handler(self))
    }
}

#[handler]
impl Base {}
