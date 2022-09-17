use crate::driver::{
    Driver, DriverCtx, DriverSkel, HyperDriverFactory, Item, ItemHandler, ItemSphere,
};
use crate::star::HyperStarSkel;
use crate::Platform;
use cosmic_universe::artifact::ArtRef;
use cosmic_universe::config::bind::BindConfig;
use cosmic_universe::kind::Kind;
use cosmic_universe::loc::Point;
use cosmic_universe::parse::bind_config;
use cosmic_universe::util::log;
use std::marker::PhantomData;
use std::str::FromStr;
use std::sync::Arc;
use cosmic_universe::wave::core::{CoreBounce, ReflectedCore};
use cosmic_universe::wave::exchange::{DirectedHandlerSelector, RootInCtx};

lazy_static! {
    static ref ROOT_BIND_CONFIG: ArtRef<BindConfig> = ArtRef::new(
        Arc::new(root_bind()),
        Point::from_str("GLOBAL::repo:1.0.0:/bind/root.bind").unwrap()
    );
}

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
    fn kind(&self) -> Kind {
        Kind::Root
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
impl DirectedHandler for RootDriver {
    async fn handle(&self, ctx: RootInCtx) -> CoreBounce {
        CoreBounce::Reflected(ReflectedCore::status(404))
    }
}

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

use cosmic_universe::wave::exchange::DirectedHandler;
use cosmic_universe::wave::RecipientSelector;
#[routes]
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
