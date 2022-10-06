use crate::driver::{
    Driver, DriverAvail, DriverCtx, DriverSkel, HyperDriverFactory, ItemHandler, ItemSphere,
};
use crate::star::HyperStarSkel;
use crate::{HyperErr, Hyperverse};
use cosmic_universe::artifact::ArtRef;
use cosmic_universe::config::bind::BindConfig;
use cosmic_universe::hyper::Assign;
use cosmic_universe::kind::Kind;
use cosmic_universe::loc::Point;
use cosmic_universe::parse::bind_config;
use cosmic_universe::util::log;
use std::str::FromStr;
use std::sync::Arc;

lazy_static! {
    static ref REPO_BIND_CONFIG: ArtRef<BindConfig> = ArtRef::new(
        Arc::new(repo_bind()),
        Point::from_str("GLOBAL::repo:1.0.0:/bind/repo.bind").unwrap()
    );
    static ref SERIES_BIND_CONFIG: ArtRef<BindConfig> = ArtRef::new(
        Arc::new(series_bind()),
        Point::from_str("GLOBAL::repo:1.0.0:/bind/bundle_series.bind").unwrap()
    );
    static ref BUNDLE_BIND_CONFIG: ArtRef<BindConfig> = ArtRef::new(
        Arc::new(series_bind()),
        Point::from_str("GLOBAL::repo:1.0.0:/bind/bundle.bind").unwrap()
    );
}

fn repo_bind() -> BindConfig {
    log(bind_config(
        r#"
    Bind(version=1.0.0)
    {
    }
    "#,
    ))
    .unwrap()
}

fn series_bind() -> BindConfig {
    log(bind_config(
        r#"
    Bind(version=1.0.0)
    {
    }
    "#,
    ))
    .unwrap()
}

fn bundle_bind() -> BindConfig {
    log(bind_config(
        r#"
    Bind(version=1.0.0)
    {
    }
    "#,
    ))
    .unwrap()
}

pub struct RepoDriverFactory;

impl RepoDriverFactory {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl<P> HyperDriverFactory<P> for RepoDriverFactory
where
    P: Hyperverse,
{
    fn kind(&self) -> Kind {
        Kind::Repo
    }

    async fn create(
        &self,
        skel: HyperStarSkel<P>,
        driver_skel: DriverSkel<P>,
        ctx: DriverCtx,
    ) -> Result<Box<dyn Driver<P>>, P::Err> {
        Ok(Box::new(RepoDriver::new()))
    }
}

pub struct RepoDriver {}

#[handler]
impl RepoDriver {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl<P> Driver<P> for RepoDriver
where
    P: Hyperverse,
{
    fn kind(&self) -> Kind {
        Kind::Repo
    }

    async fn item(&self, point: &Point) -> Result<ItemSphere<P>, P::Err> {
        Ok(ItemSphere::Handler(Box::new(Repo)))
    }
}

pub struct Repo;

#[handler]
impl Repo {}

#[async_trait]
impl<P> ItemHandler<P> for Repo
where
    P: Hyperverse,
{
    async fn bind(&self) -> Result<ArtRef<BindConfig>, P::Err> {
        Ok(REPO_BIND_CONFIG.clone())
    }
}

pub struct BundleSeriesDriverFactory;

impl BundleSeriesDriverFactory {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl<P> HyperDriverFactory<P> for BundleSeriesDriverFactory
where
    P: Hyperverse,
{
    fn kind(&self) -> Kind {
        Kind::BundleSeries
    }

    async fn create(
        &self,
        skel: HyperStarSkel<P>,
        driver_skel: DriverSkel<P>,
        ctx: DriverCtx,
    ) -> Result<Box<dyn Driver<P>>, P::Err> {
        Ok(Box::new(BundleSeriesDriver::new()))
    }
}

pub struct BundleSeriesDriver {}

#[handler]
impl BundleSeriesDriver {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl<P> Driver<P> for BundleSeriesDriver
where
    P: Hyperverse,
{
    fn kind(&self) -> Kind {
        Kind::BundleSeries
    }

    async fn item(&self, point: &Point) -> Result<ItemSphere<P>, P::Err> {
        Ok(ItemSphere::Handler(Box::new(BundleSeries)))
    }
}

pub struct BundleSeries;

#[handler]
impl BundleSeries {}

#[async_trait]
impl<P> ItemHandler<P> for BundleSeries
where
    P: Hyperverse,
{
    async fn bind(&self) -> Result<ArtRef<BindConfig>, P::Err> {
        Ok(SERIES_BIND_CONFIG.clone())
    }
}

pub struct BundleDriverFactory;

impl BundleDriverFactory {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl<P> HyperDriverFactory<P> for BundleDriverFactory
where
    P: Hyperverse,
{
    fn kind(&self) -> Kind {
        Kind::Bundle
    }

    async fn create(
        &self,
        skel: HyperStarSkel<P>,
        driver_skel: DriverSkel<P>,
        ctx: DriverCtx,
    ) -> Result<Box<dyn Driver<P>>, P::Err> {
        Ok(Box::new(BundleDriver::new()))
    }
}

pub struct BundleDriver {}

#[handler]
impl BundleDriver {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl<P> Driver<P> for BundleDriver
where
    P: Hyperverse,
{
    fn kind(&self) -> Kind {
        Kind::Bundle
    }

    async fn item(&self, point: &Point) -> Result<ItemSphere<P>, P::Err> {
        Ok(ItemSphere::Handler(Box::new(Bundle)))
    }

    async fn assign(&self, assign: Assign) -> Result<(), P::Err> {
        if !assign.details.stub.point.is_artifact_bundle() {
            Err(P::Err::new(
                "invalid Artifact Bundle Point (must end with a version number)",
            ))?;
        }

        Ok(())
    }
}

pub struct Bundle;

#[handler]
impl Bundle {}

#[async_trait]
impl<P> ItemHandler<P> for Bundle
where
    P: Hyperverse,
{
    async fn bind(&self) -> Result<ArtRef<BindConfig>, P::Err> {
        Ok(BUNDLE_BIND_CONFIG.clone())
    }
}
