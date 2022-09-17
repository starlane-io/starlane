use crate::driver::{Driver, DriverCtx, DriverSkel, HyperDriverFactory, ItemHandler, ItemSphere};
use crate::star::HyperStarSkel;
use crate::{DriverFactory, Platform};
use cosmic_universe::artifact::ArtRef;
use cosmic_universe::config::bind::BindConfig;
use cosmic_universe::kind::Kind;
use cosmic_universe::loc::Point;
use cosmic_universe::parse::bind_config;
use cosmic_universe::util::log;
use cosmic_universe::wave::{
    CoreBounce, DirectedHandler, DirectedHandlerSelector, RecipientSelector, ReflectedCore,
    RootInCtx,
};
use std::str::FromStr;
use std::sync::Arc;
lazy_static! {
    static ref SPACE_BIND_CONFIG: ArtRef<BindConfig> = ArtRef::new(
        Arc::new(space_bind()),
        Point::from_str("GLOBAL::repo:1.0.0:/bind/space.bind").unwrap()
    );
}

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
impl<P> HyperDriverFactory<P> for SpaceDriverFactory
where
    P: Platform,
{
    fn kind(&self) -> Kind {
        Kind::Space
    }

    async fn create(
        &self,
        skel: HyperStarSkel<P>,
        driver_skel: DriverSkel<P>,
        ctx: DriverCtx,
    ) -> Result<Box<dyn Driver<P>>, P::Err> {
        Ok(Box::new(SpaceDriver))
    }
}

pub struct SpaceDriver;

#[routes]
impl SpaceDriver {}

#[async_trait]
impl<P> Driver<P> for SpaceDriver
where
    P: Platform,
{
    fn kind(&self) -> Kind {
        Kind::Space
    }

    async fn item(&self, point: &Point) -> Result<ItemSphere<P>, P::Err> {
        Ok(ItemSphere::Handler(Box::new(Space)))
    }
}

pub struct Space;

#[routes]
impl Space {}

#[async_trait]
impl<P> ItemHandler<P> for Space
where
    P: Platform,
{
    async fn bind(&self) -> Result<ArtRef<BindConfig>, P::Err> {
        Ok(SPACE_BIND_CONFIG.clone())
    }
}
