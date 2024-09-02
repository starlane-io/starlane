use async_trait::async_trait;
use lazy_static::lazy_static;
use cosmic_space::artifact::ArtRef;
use cosmic_space::config::bind::BindConfig;
use cosmic_space::kind::Kind;
use cosmic_space::loc::Layer;
use cosmic_space::parse::bind_config;
use cosmic_space::util::log;

lazy_static! {
    static ref DEFAULT_BIND: ArtRef<BindConfig> = ArtRef::new(
        Arc::new(default_bind()),
        Point::from_str("GLOBAL::repo:1.0.0:/bind/default.bind").unwrap()
    );
    static ref DRIVER_BIND: ArtRef<BindConfig> = ArtRef::new(
        Arc::new(driver_bind()),
        Point::from_str("GLOBAL::repo:1.0.0:/bind/driver.bind").unwrap()
    );
}


fn driver_bind() -> BindConfig {
    log(bind_config(
        r#" Bind(version=1.0.0) {

       Route<Hyp<Assign>> -> (()) => &;

    } "#,
    ))
        .unwrap()
}

fn default_bind() -> BindConfig {
    log(bind_config(r#" Bind(version=1.0.0) { } "#)).unwrap()
}


#[async_trait]
pub trait Driver<P>: Send + Sync
where P: Cosmos,
{
    fn kind(&self) -> Kind;

    fn layer(&self) -> Layer {
        Layer::Core
    }

    fn avail(&self) -> DriverAvail {
        DriverAvail::External
    }

    fn bind(&self) -> ArtRef<BindConfig> {
        DRIVER_BIND.clone()
    }

    async fn init(&mut self, skel: DriverSkel<P>, ctx: DriverCtx) -> Result<(), P::Err> {
        skel.logger
            .result(skel.status_tx.send(DriverStatus::Ready).await)
            .unwrap_or_default();
        Ok(())
    }

    async fn item(&self, point: &Point) -> Result<ItemSphere<P>, P::Err>;

    async fn handler(&self) -> Box<dyn DriverHandler<P>> {
        Box::new(DefaultDriverHandler::restore())
    }

    /// This is sorta a hack, it only works for DriverDriver
    async fn add_driver(&self, _driver: DriverApi<P>) {}
}




#[derive(Clone, Eq, PartialEq)]
pub enum DriverAvail {
    Internal,
    External,
}