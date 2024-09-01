use async_trait::async_trait;
use cosmic_hyperspace::star::HyperStarSkel;
use cosmic_hyperspace::Cosmos;
use cosmic_macros::handler;
use cosmic_space::artifact::ArtRef;
use cosmic_space::config::bind::BindConfig;
use cosmic_space::kind::{BaseKind, Kind};
use cosmic_space::parse::bind_config;
use cosmic_space::point::Point;
use cosmic_space::selector::KindSelector;
use cosmic_space::util::log;
use lazy_static::lazy_static;
use std::str::FromStr;
use std::sync::Arc;
use cosmic_hyperspace::driver::{Driver, DriverCtx, DriverSkel, HyperDriverFactory, ItemHandler, ItemSphere};

lazy_static! {
    static ref FILESYSTSEM_BIND_CONFIG: ArtRef<BindConfig> = ArtRef::new(
        Arc::new(bind()),
        Point::from_str("GLOBAL::repo:1.0.0:/bind/filesystem.bind").unwrap()
    );
}

fn bind() -> BindConfig {
    log(bind_config(
        r#"
    Bind(version=1.0.0)
    {
    }
    "#,
    ))
    .unwrap()
}

pub struct FilesystemFactory;

impl FilesystemFactory {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl<P> HyperDriverFactory<P> for FilesystemFactory
where
    P: Cosmos,
{
    fn kind(&self) -> KindSelector {
        KindSelector::from_base(BaseKind::FileSystem)
    }

    async fn create(
        &self,
        skel: HyperStarSkel<P>,
        driver_skel: DriverSkel<P>,
        ctx: DriverCtx,
    ) -> Result<Box<dyn Driver<P>>, P::Err> {
        Ok(Box::new(FileSystemDriver))
    }
}

pub struct FileSystemDriver;

#[async_trait]
impl<P> Driver<P> for FileSystemDriver
where
    P: Cosmos,
{
    fn kind(&self) -> Kind {
        Kind::Space
    }

    async fn item(&self, point: &Point) -> Result<ItemSphere<P>, P::Err> {
        Ok(ItemSphere::Handler(Box::new(FileSystem)))
    }
}

pub struct FileSystem;

#[handler]
impl FileSystem {}

#[async_trait]
impl<P> ItemHandler<P> for FileSystem
where
    P: Cosmos,
{
    async fn bind(&self) -> Result<ArtRef<BindConfig>, P::Err> {
        Ok(FILESYSTSEM_BIND_CONFIG.clone())
    }
}
