use crate::driver::{
    Driver, DriverAvail, DriverCtx, DriverErr, DriverSkel, HyperDriverFactory, Particle,
    ParticleSphere, StdParticleErr,
};

use crate::base::Platform;
use crate::star::HyperStarSkel;
use async_trait::async_trait;
use starlane_macros::{handler, DirectedHandler};
use starlane_space::kind::{BaseKind, Kind};
use starlane_space::point::Point;
use starlane_space::selector::KindSelector;
use starlane_space::wave::exchange::asynch::DirectedHandler;
use std::str::FromStr;

pub struct FileStoreDriverFactory {
    pub avail: DriverAvail,
}

impl FileStoreDriverFactory {
    pub fn new(avail: DriverAvail) -> Self {
        Self { avail }
    }
}

#[async_trait]
impl HyperDriverFactory for FileStoreDriverFactory {
    fn kind(&self) -> Kind {
        Kind::FileStore
    }

    fn selector(&self) -> KindSelector {
        KindSelector::from_base(BaseKind::FileStore)
    }

    async fn create(
        &self,
        _: HyperStarSkel,
        _: DriverSkel,
        _: DriverCtx,
    ) -> Result<Box<dyn Driver>, DriverErr> {
        Ok(Box::new(FileStoreDriver::new(self.avail.clone())))
    }
}

pub struct FileStoreDriver {
    pub avail: DriverAvail,
}

impl FileStoreDriver {
    pub fn new(avail: DriverAvail) -> Self {
        Self { avail }
    }
}

#[async_trait]
impl Driver for FileStoreDriver {
    fn kind(&self) -> Kind {
        Kind::FileStore
    }

    async fn particle(&self, point: &Point) -> Result<ParticleSphere, DriverErr> {
        let base = FileStore::restore((), (), ());

        Ok(base.sphere()?)
    }
}

#[derive(DirectedHandler)]
pub struct FileStore;

impl Particle for FileStore {
    type Skel = ();
    type Ctx = ();
    type State = ();
    type Err = StdParticleErr;

    fn restore(_: Self::Skel, _: Self::Ctx, _: Self::State) -> Self {
        FileStore
    }

    fn sphere(self) -> Result<ParticleSphere, Self::Err> {
        Ok(ParticleSphere::new_handler(self))
    }
}

#[handler]
impl FileStore {}
