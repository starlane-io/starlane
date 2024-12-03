use crate::hyperspace::driver::{
    Driver, DriverAvail, DriverCtx, DriverErr, DriverSkel, HyperDriverFactory, Particle,
    ParticleSphere, ParticleSphereInner, StdParticleErr,
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
use crate::space::wave::exchange::asynch::DirectedHandler;
use async_trait::async_trait;
use once_cell::sync::Lazy;
use starlane_macros::{handler, DirectedHandler};
use std::str::FromStr;
use std::sync::Arc;

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
