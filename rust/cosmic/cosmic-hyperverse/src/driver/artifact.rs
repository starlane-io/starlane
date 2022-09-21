use crate::driver::{
    Driver, DriverAvail, DriverCtx, DriverSkel, HyperDriverFactory, ItemHandler, ItemSphere,
};
use crate::star::HyperStarSkel;
use crate::{HyperErr, Hyperverse};
use acid_store::repo::key::KeyRepo;
use acid_store::repo::OpenOptions;
use acid_store::store::MemoryConfig;
use cosmic_universe::artifact::ArtRef;
use cosmic_universe::command::common::{SetProperties, StateSrc};
use cosmic_universe::command::direct::create::{
    Create, KindTemplate, PointSegTemplate, PointTemplate, Strategy, Template,
};
use cosmic_universe::config::bind::BindConfig;
use cosmic_universe::err::UniErr;
use cosmic_universe::hyper::Assign;
use cosmic_universe::kind::{ArtifactSubKind, Kind};
use cosmic_universe::loc::{Point, ToBaseKind};
use cosmic_universe::parse::bind_config;
use cosmic_universe::particle::PointKind;
use cosmic_universe::substance::{Bin, Substance};
use cosmic_universe::util::log;
use cosmic_universe::wave::core::DirectedCore;
use std::cmp::Ordering;
use std::collections::HashSet;
use std::fs::File;
use std::io::Read;
use std::io::Write;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use tempdir::TempDir;

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
        Ok(Box::new(RepoDriver::new(skel)))
    }
}

pub struct RepoDriver<P>
where
    P: Hyperverse,
{
    skel: HyperStarSkel<P>,
}

#[handler]
impl<P> RepoDriver<P>
where
    P: Hyperverse,
{
    pub fn new(skel: HyperStarSkel<P>) -> Self {
        Self { skel }
    }
}

#[async_trait]
impl<P> Driver<P> for RepoDriver<P>
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

fn file_repo() -> Result<KeyRepo<String>, UniErr> {
    let config = acid_store::store::DirectoryConfig {
        path: PathBuf::from("./data/artifacts"),
    };

    match OpenOptions::new()
        .mode(acid_store::repo::OpenMode::Create)
        .open(&config)
    {
        Ok(repo) => Ok(repo),
        Err(err) => return Err(UniErr::new(500u16, err.to_string())),
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
println!();
println!("!!!!! BundleDriver Received ASSIGN! {} ----------", assign.details.stub.kind.to_string());
println!();
        let state = match &assign.state {
            StateSrc::Substance(data) => data.clone(),
            StateSrc::None => {
println!("ArtifactBundle cannot be stateless");
                return Err("ArtifactBundle cannot be stateless".into())
            },
        };
println!("...");
        if let Substance::Bin(zip) = (*state).clone() {
            let temp_dir = TempDir::new("zipcheck")?;
            let temp_path = temp_dir.path().clone();
            let file_path = temp_path.with_file_name("file.zip");
            let mut file = File::create(file_path.as_path())?;
            file.write_all(zip.as_slice())?;

println!("wrote to tmpfile");
            let file = File::open(file_path.as_path())?;
            let mut archive = zip::ZipArchive::new(file)?;
            let mut artifacts = vec![];
            for i in 0..archive.len() {
                let file = archive.by_index(i).unwrap();
                if !file.name().ends_with("/") {
                    artifacts.push(file.name().to_string())
                }
            }

println!("whatup artifact?");
            let mut point_and_kind_set = HashSet::new();
            for artifact in artifacts {
                let mut path = String::new();
                let segments = artifact.split("/");
                let segments: Vec<&str> = segments.collect();
                for (index, segment) in segments.iter().enumerate() {
                    path.push_str(segment);
                    if index < segments.len() - 1 {
                        path.push_str("/");
                    }
                    let point = Point::from_str(
                        format!(
                            "{}:/{}",
                            assign.details.stub.point.to_string(),
                            path.as_str()
                        )
                        .as_str(),
                    )?;
                    let kind = if index < segments.len() - 1 {
                        Kind::Artifact(ArtifactSubKind::Dir)
                    } else {
                        Kind::Artifact(ArtifactSubKind::Raw)
                    };
                    let point_and_kind = PointKind { point, kind };
                    point_and_kind_set.insert(point_and_kind);
                }
            }

            let root_point_and_kind = PointKind {
                point: Point::from_str(
                    format!("{}:/", assign.details.stub.point.to_string()).as_str(),
                )?,
                kind: Kind::Artifact(ArtifactSubKind::Dir),
            };

            point_and_kind_set.insert(root_point_and_kind);

            let mut point_and_kind_set: Vec<PointKind> = point_and_kind_set.into_iter().collect();

            // shortest first will ensure that dirs are created before files
            point_and_kind_set.sort_by(|a, b| {
                if a.point.to_string().len() > b.point.to_string().len() {
                    Ordering::Greater
                } else if a.point.to_string().len() < b.point.to_string().len() {
                    Ordering::Less
                } else {
                    Ordering::Equal
                }
            });

            {
                tokio::spawn(async move {
                    for point_and_kind in point_and_kind_set {
                        let parent = point_and_kind.point.parent().expect("expected parent");

                        let state = match point_and_kind.kind {
                            Kind::Artifact(ArtifactSubKind::Dir) => StateSrc::None,
                            Kind::Artifact(_) => {
                                let mut path = point_and_kind
                                    .point
                                    .filepath()
                                    .expect("expecting non Dir artifact to have a filepath");
                                // convert to relative path
                                path.remove(0);
                                match archive.by_name(path.as_str()) {
                                    Ok(mut file) => {
                                        let mut buf = vec![];
                                        file.read_to_end(&mut buf);
                                        let bin = Arc::new(buf);
                                        let payload = Substance::Bin(bin);
                                        StateSrc::Substance(Box::new(payload))
                                    }
                                    Err(err) => {
                                        eprintln!("Artifact archive error: {}", err.to_string());
                                        StateSrc::None
                                    }
                                }
                            }
                            _ => {
                                panic!("unexpected knd");
                            }
                        };

                        /*
                                               let create = Create {
                                                   template: Template {
                                                       point: PointTemplate {
                                                           parent: parent.clone(),
                                                           child_segment_template: PointSegTemplate::Exact(
                                                               point_and_kind
                                                                   .point
                                                                   .last_segment()
                                                                   .expect("expected final segment")
                                                                   .to_string(),
                                                           ),
                                                       },
                                                       kind: KindTemplate {
                                                           base: point_and_kind.kind.to_base(),
                                                           sub: point_and_kind.kind.sub().into(),
                                                           specific: None,
                                                       },
                                                   },
                                                   state,
                                                   properties: SetProperties::new(),
                                                   strategy: Strategy::Commit,
                                               };

                                               // this is where things get SENT to other parts... do we need it?
                                               let core : DirectedCore = create.into();
                                               let request =
                                                   ReqShell::new(core, assign.details.stub.point.clone(), parent);
                                               let response = skel.messaging_api.request(request).await;

                        */
                    }
                });
            }
        } else {
            return Err("ArtifactBundle Manager expected Bin payload".into());
        }

        let mut object = file_repo()?.insert(assign.details.stub.point.to_string());
        object.write_all(bincode::serialize(&state)?.as_slice())?;
        object.commit()?;
        //        self.store.put(assign.details.stub.point, *state).await?;

        // need to unzip and create Artifacts for each...

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
