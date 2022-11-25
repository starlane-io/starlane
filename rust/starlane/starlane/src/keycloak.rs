use std::str::FromStr;
use cosmic_hyperspace::Cosmos;
use cosmic_hyperspace::driver::{Driver, DriverCtx, DriverHandler, DriverSkel, HyperDriverFactory, HyperSkel, Item, ItemHandler, ItemSkel, ItemSphere};
use cosmic_hyperspace::star::HyperStarSkel;
use cosmic_space::hyper::HyperSubstance;
use cosmic_space::kind::{UserVariant, BaseKind, Kind, Specific};
use cosmic_space::point::Point;
use cosmic_space::selector::KindSelector;
use cosmic_space::substance::Substance;
use cosmic_space::wave::exchange::asynch::InCtx;
use cosmic_hyperspace::err::HyperErr;
use cosmic_space::artifact::ArtRef;
use cosmic_space::config::bind::BindConfig;
use cosmic_space::parse::bind_config;
use cosmic_space::util::log;
use std::sync::Arc;

lazy_static! {
    static ref AUTH_BIND_CONFIG: ArtRef<BindConfig> = ArtRef::new(
        Arc::new(auth_bind()),
        Point::from_str("GLOBAL::repo:1.0.0:/bind/auth.bind").unwrap()
    );
}

fn auth_bind() -> BindConfig {
    log(bind_config(
        r#"
    Bind(version=1.0.0)
    {
        Route -> {
        }
    }
    "#,
    ))
        .unwrap()
}

pub struct KeycloakDriverFactory;

impl KeycloakDriverFactory {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl<P> HyperDriverFactory<P> for KeycloakDriverFactory
    where
        P: Cosmos,
{
    fn kind(&self) -> KindSelector {
        KindSelector::from_base(BaseKind::User)
    }

    async fn create(
        &self,
        skel: HyperStarSkel<P>,
        driver_skel: DriverSkel<P>,
        ctx: DriverCtx,
    ) -> Result<Box<dyn Driver<P>>, P::Err> {
        let skel = HyperSkel::new(skel, driver_skel);
        Ok(Box::new(KeycloakDriver::new(skel, ctx)))
    }
}


 pub struct KeycloakDriver<P>
where
    P: Cosmos,
{
    skel: HyperSkel<P>,
    ctx: DriverCtx,
}

#[handler]
impl<P> KeycloakDriver<P>
where
    P: Cosmos,
{
    pub fn new(skel: HyperSkel<P>, ctx: DriverCtx) -> Self {
        Self { skel, ctx }
    }
}

#[async_trait]

impl<P> Driver<P> for KeycloakDriver<P>
where
    P: Cosmos,
{
    fn kind(&self) -> Kind {
        Kind::User(UserVariant::OAuth(Specific::from_str("starlane.io:starlane.io:keycloak:community:1.0.0").unwrap()))
    }

    async fn item(&self, point: &Point) -> Result<ItemSphere<P>, P::Err> {
        let record = self.skel.driver.locate(point).await?;
        let skel = ItemSkel::new(point.clone(), record.details.stub.kind, self.skel.driver.clone());
        Ok(ItemSphere::Handler(Box::new(Keycloak::restore(skel,(),()))))
    }

    async fn handler(&self) -> Box<dyn DriverHandler<P>> {
        Box::new(KeycloakDriverHandler::restore(
            self.skel.clone(),
            self.ctx.clone(),
        ))
    }
}

pub struct KeycloakDriverHandler<P>
where
    P: Cosmos,
{
    skel: HyperSkel<P>,
    ctx: DriverCtx,
}

impl<P> KeycloakDriverHandler<P>
where
    P: Cosmos,
{
    fn restore(skel: HyperSkel<P>, ctx: DriverCtx) -> Self {
        Self { skel, ctx }
    }
}

impl<P> DriverHandler<P> for KeycloakDriverHandler<P> where P: Cosmos {}

#[handler]
impl<P> KeycloakDriverHandler<P>
where
    P: Cosmos,
{

    #[route("Hyp<Assign>")]
    async fn assign(&self, ctx: InCtx<'_, HyperSubstance>) -> Result<(), P::Err> {
        Ok(())
    }
}


pub struct Keycloak<P>
    where
        P: Cosmos,
{
    skel: ItemSkel<P>,
}

#[handler]
impl<P> Keycloak<P>
    where
        P: Cosmos,
{

}

impl<P> Item<P> for Keycloak<P>
    where
        P: Cosmos,
{
    type Skel = ItemSkel<P>;
    type Ctx = ();
    type State = ();

    fn restore(skel: Self::Skel, ctx: Self::Ctx, state: Self::State) -> Self {
        Self { skel }
    }
}

#[async_trait]
impl <P> ItemHandler<P> for Keycloak<P> where P: Cosmos{
    async fn bind(&self) -> Result<ArtRef<BindConfig>, P::Err> {
        Ok( AUTH_BIND_CONFIG.clone() )
    }
}