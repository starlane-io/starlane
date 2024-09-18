use crate::hyper::space::driver::{
    Driver, DriverCtx, DriverSkel, HyperDriverFactory, ItemHandler, ItemSphere,
};
use crate::hyper::space::star::HyperStarSkel;
use crate::hyper::space::Cosmos;
use once_cell::sync::Lazy;
use starlane_space::artifact::ArtRef;
use starlane_space::config::bind::BindConfig;
use starlane_space::kind::{BaseKind, Kind};
use starlane_space::parse::bind_config;
use starlane_space::point::Point;
use starlane_space::selector::KindSelector;
use starlane_space::util::log;
use std::str::FromStr;
use std::sync::Arc;

static SPACE_BIND_CONFIG: Lazy<ArtRef<BindConfig>> = Lazy::new(|| {
    ArtRef::new(
        Arc::new(space_bind()),
        Point::from_str("GLOBAL::repo:1.0.0:/bind/space.bind").unwrap(),
    )
});

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
    P: Cosmos,
{
    fn kind(&self) -> KindSelector {
        KindSelector::from_base(BaseKind::Space)
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

#[async_trait]
impl<P> Driver<P> for SpaceDriver
where
    P: Cosmos,
{
    fn kind(&self) -> Kind {
        Kind::Space
    }

    async fn item(&self, point: &Point) -> Result<ItemSphere<P>, P::Err> {
        Ok(ItemSphere::Handler(Box::new(Space)))
    }
}

pub struct Space;

#[handler]
impl Space {}

#[async_trait]
impl<P> ItemHandler<P> for Space
where
    P: Cosmos,
{
    async fn bind(&self) -> Result<ArtRef<BindConfig>, P::Err> {
        Ok(SPACE_BIND_CONFIG.clone())
    }
}
