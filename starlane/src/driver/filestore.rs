use crate::driver::{Driver, DriverAvail, DriverCtx, DriverErr, DriverSkel, HyperDriverFactory, Particle, ParticleSphere, ParticleSphereInner, StdParticleErr};

pub use starlane_space as starlane;

use crate::platform::Platform;
use crate::starlane_hyperspace::hyperspace::star::HyperStarSkel;
use once_cell::sync::Lazy;
use starlane::space::artifact::ArtRef;
use starlane::space::config::bind::BindConfig;
use starlane::space::parse::bind_config;
use starlane::space::point::Point;
use starlane::space::selector::KindSelector;
use starlane::space::util::log;
use starlane::space::wave::exchange::asynch::DirectedHandler;
use std::str::FromStr;
use std::sync::Arc;
use starlane::space::kind::{BaseKind, Kind};

pub struct FileStoreDriverFactory {
    pub avail: DriverAvail,
}

impl FileStoreDriverFactory {
    pub fn new(avail: DriverAvail) -> Self {
        Self { avail }
    }
}

#[async_trait]
impl HyperDriverFactory for FileStoreDriverFactory

{
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
impl Driver for FileStoreDriver

{
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


    fn sphere(self) -> Result<ParticleSphere,Self::Err>{
        Ok(ParticleSphere::new_handler(self))
    }
}

#[handler]
impl FileStore {}


