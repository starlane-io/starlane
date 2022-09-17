use std::str::FromStr;
use std::sync::Arc;
use cosmic_universe::artifact::ArtRef;
use cosmic_universe::config::config::bind::BindConfig;
use cosmic_universe::id::id::{Kind, Point};
use cosmic_universe::parse::bind_config;
use cosmic_universe::util::log;
use cosmic_universe::wave::{CoreBounce, DirectedHandler, DirectedHandlerSelector, RecipientSelector, ReflectedCore, RootInCtx};
use crate::{DriverFactory, Platform};
use crate::driver::{Driver, DriverAvail, DriverCtx, DriverSkel, HyperDriverFactory, ItemHandler, ItemSphere};
use crate::star::HyperStarSkel;
lazy_static! {
    static ref BASE_BIND_CONFIG: ArtRef<BindConfig> = ArtRef::new(
        Arc::new(base_bind()),
        Point::from_str("GLOBAL::repo:1.0.0:/bind/base.bind").unwrap()
    );
}


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

pub struct BaseDriverFactory{
    pub avail: DriverAvail
}

impl BaseDriverFactory {
    pub fn new(avail: DriverAvail) -> Self {
        Self {
            avail
        }
    }
}

#[async_trait]
impl <P> HyperDriverFactory<P> for BaseDriverFactory where P: Platform {
    fn kind(&self) -> Kind {
        Kind::Base
    }

    async fn create(&self, skel: HyperStarSkel<P>, driver_skel: DriverSkel<P>, ctx: DriverCtx) -> Result<Box<dyn Driver<P>>, P::Err> {
        Ok(Box::new(BaseDriver::new(self.avail.clone())))
    }

}

pub struct BaseDriver {
    pub avail: DriverAvail
}

#[routes]
impl BaseDriver {

    pub fn new(avail: DriverAvail) -> Self {
        Self {
            avail
        }
    }
}


#[async_trait]
impl <P> Driver<P> for BaseDriver where P: Platform {
    fn kind(&self) -> Kind {
        Kind::Base
    }

    async fn item(&self, point: &Point) -> Result<ItemSphere<P>, P::Err> {
        Ok(ItemSphere::Handler(Box::new(Base)))
    }
}


pub struct Base;

#[routes]
impl Base {}

#[async_trait]
impl <P> ItemHandler<P> for Base where P: Platform {
    async fn bind(&self) -> Result<ArtRef<BindConfig>, P::Err> {
        Ok(BASE_BIND_CONFIG.clone())
    }
}


