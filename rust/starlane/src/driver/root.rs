use crate::driver::{
    Driver, DriverCtx, DriverSkel, HyperDriverFactory, Item, ItemHandler, ItemSphere,
};
pub use starlane_space as starlane;

use crate::hyperspace::platform::Platform;
use crate::hyperspace::star::HyperStarSkel;
use once_cell::sync::Lazy;
use starlane::space::artifact::ArtRef;
use starlane::space::config::bind::BindConfig;
use starlane::space::kind::{BaseKind, Kind};
use starlane::space::parse::bind_config;
use starlane::space::point::Point;
use starlane::space::selector::KindSelector;
use starlane::space::util::log;
use starlane::space::wave::exchange::asynch::DirectedHandler;
use std::marker::PhantomData;
use std::str::FromStr;
use std::sync::Arc;

static ROOT_BIND_CONFIG: Lazy<ArtRef<BindConfig>> = Lazy::new(|| {
    ArtRef::new(
        Arc::new(root_bind()),
        Point::from_str("GLOBAL::repo:1.0.0:/bind/root.bind").unwrap(),
    )
});

fn root_bind() -> BindConfig {
    log(bind_config(
        r#"
    Bind(version=1.0.0)
    { }
    "#,
    ))
    .unwrap()
}

pub struct RootDriverFactory;

impl RootDriverFactory {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl<P> HyperDriverFactory<P> for RootDriverFactory
where
    P: Platform,
{
    fn kind(&self) -> KindSelector {
        KindSelector::from_base(BaseKind::Root)
    }

    async fn create(
        &self,
        skel: HyperStarSkel<P>,
        driver_skel: DriverSkel<P>,
        ctx: DriverCtx,
    ) -> Result<Box<dyn Driver<P>>, P::Err> {
        Ok(Box::new(RootDriver {}))
    }
}

pub struct RootDriver;

#[async_trait]
impl<P> Driver<P> for RootDriver
where
    P: Platform,
{
    fn kind(&self) -> Kind {
        Kind::Root
    }

    async fn item(&self, point: &Point) -> Result<ItemSphere<P>, P::Err> {
        Ok(ItemSphere::Handler(Box::new(Root::restore((), (), ()))))
    }
}

pub struct Root<P>
where
    P: Platform,
{
    phantom: PhantomData<P>,
}

impl<P> Root<P>
where
    P: Platform,
{
    pub fn new() -> Self {
        Self {
            phantom: PhantomData::default(),
        }
    }
}

impl<P> Item<P> for Root<P>
where
    P: Platform,
{
    type Skel = ();
    type Ctx = ();
    type State = ();

    fn restore(skel: Self::Skel, ctx: Self::Ctx, state: Self::State) -> Self {
        Self::new()
    }
}

#[handler]
impl<P> Root<P> where P: Platform {}

#[async_trait]
impl<P> ItemHandler<P> for Root<P>
where
    P: Platform,
{
    async fn bind(&self) -> Result<ArtRef<BindConfig>, P::Err> {
        Ok(ROOT_BIND_CONFIG.clone())
    }
}
