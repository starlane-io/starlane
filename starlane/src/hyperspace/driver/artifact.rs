use crate::hyperspace::driver::{
    Driver, DriverCtx, DriverErr, DriverHandler, DriverSkel, DriverStatus, HyperDriverFactory,
    HyperSkel, Particle, ParticleSkel, ParticleSphere, ParticleSphereInner, StdParticleErr,
};
use crate::hyperspace::executor::dialect::filestore::FileStoreIn;
use crate::hyperspace::platform::Platform;
use crate::hyperspace::service::{FileStoreService, Service, ServiceKind, ServiceRunner, ServiceSelector};
use crate::hyperspace::star::HyperStarSkel;
use async_trait::async_trait;
use once_cell::sync::Lazy;
use starlane_macros::{handler, DirectedHandler};
use starlane_space::artifact::ArtRef;
use starlane_space::command::common::{SetProperties, StateSrc};
use starlane_space::command::direct::create::{
    Create, KindTemplate, PointSegTemplate, PointTemplate, Strategy, Template,
};
use starlane_space::config::bind::BindConfig;
use starlane_space::err::SpaceErr;
use starlane_space::hyper::HyperSubstance;
use starlane_space::kind::{ArtifactSubKind, BaseKind, Kind};
use starlane_space::loc::ToBaseKind;
use starlane_space::parse::bind_config;
use starlane_space::particle::PointKind;
use starlane_space::point::Point;
use starlane_space::selector::KindSelector;
use starlane_space::substance::Substance;
use starlane_space::util::{log, IdSelector};
use starlane_space::wave::exchange::asynch::InCtx;
use starlane_space::wave::{DirectedProto, Pong, Wave};
use std::marker::PhantomData;
use std::str::FromStr;
use std::sync::Arc;
use tempdir::TempDir;
use tracing::Instrument;

pub struct RepoDriverFactory {}

impl RepoDriverFactory {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl HyperDriverFactory for RepoDriverFactory {
    fn kind(&self) -> Kind {
        Kind::Repo
    }

    fn selector(&self) -> KindSelector {
        KindSelector::from_base(BaseKind::Repo)
    }

    async fn create(
        &self,
        _: HyperStarSkel,
        skel: DriverSkel,
        _: DriverCtx,
    ) -> Result<Box<dyn Driver>, DriverErr> {
        let service = skel.select_service(ServiceKind::FileStore).await?;

        let filestore = service.filestore()?;

        Ok(Box::new(RepoDriver::new(skel, filestore)))
    }
}

pub struct RepoDriver {
    skel: DriverSkel,
    filestore: FileStoreService,
}

impl RepoDriver {
    pub fn new(skel: DriverSkel, filestore: FileStoreService) -> Self {
        Self { skel, filestore }
    }
}

#[handler]
impl RepoDriver {}

#[async_trait]
impl Driver for RepoDriver {
    async fn init(&mut self, skel: DriverSkel, ctx: DriverCtx) -> Result<(), DriverErr> {
        skel.logger
            .result(skel.status_tx.send(DriverStatus::Init).await)
            .unwrap_or_default();

        self.filestore.execute(FileStoreIn::Init).await?;

        skel.logger
            .result(skel.status_tx.send(DriverStatus::Ready).await)
            .unwrap_or_default();
        Ok(())
    }
    fn kind(&self) -> Kind {
        Kind::Repo
    }

    async fn particle(&self, point: &Point) -> Result<ParticleSphere, DriverErr> {
        let filestore = self.filestore.sub_root(point.md5().into()).await?;

        let repo = Repo::restore((), (), filestore);
        Ok(repo.sphere()?)
    }
}

impl Particle for Repo {
    type Skel = ();
    type Ctx = ();
    type State = FileStoreService;
    type Err = StdParticleErr;

    fn restore(skel: Self::Skel, ctx: Self::Ctx, state: Self::State) -> Self {
        Self { filestore: state }
    }

    fn sphere(self) -> Result<ParticleSphere, Self::Err> {
        Ok(ParticleSphere::new_handler(self))
    }
}

/*
fn store() -> Result<ValueRepo<String>, UniErr> {
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
 */

#[derive(DirectedHandler)]
pub struct Repo {
    filestore: FileStoreService,
}

impl Repo {}

#[handler]
impl Repo {}

/*
pub struct BundleSeriesDriverFactory;

impl BundleSeriesDriverFactory {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl<P,S> HyperDriverFactory for BundleSeriesDriverFactory

{
    fn kind(&self) -> KindSelector {
        KindSelector::from_base(BaseKind::BundleSeries)
    }

    async fn create(
        &self,
        skel: HyperStarSkel,
        driver_skel: DriverSkel,
        ctx: DriverCtx,
    ) -> Result<Box<dyn Driver>, DriverErr> {
        Ok(Box::new(BundleSeriesDriver::new(ctx)))
    }
}

pub struct BundleSeriesDriver  {
    ctx: DriverCtx,
}

#[handler]
impl  BundleSeriesDriver {
    pub fn new(ctx: DriverCtx) -> Self {
        Self {
            ctx
        }
    }
}

#[async_trait]
impl Driver for BundleSeriesDriver

{
    fn kind(&self) -> Kind {
        Kind::BundleSeries
    }

    async fn item(&self, point: &Point) -> Result<ItemSphere, DriverErr> {
        Ok(ItemSphere::Handler(Box::new(BundleSeries::new(self.ctx.clone()))))
    }
}

pub struct BundleSeries  {
    ctx: DriverCtx
}

impl  BundleSeries {
   pub fn new( ctx: DriverCtx) -> BundleSeries{
       Self {
           ctx
       }
   }
}

#[handler]
impl  BundleSeries  {

}

#[async_trait]
impl ItemHandler for BundleSeries

{
    async fn bind(&self) -> Result<ArtRef<BindConfig>, DriverErr> {
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
impl HyperDriverFactory for BundleDriverFactory

{
    fn kind(&self) -> KindSelector {
        KindSelector::from_base(BaseKind::Bundle)
    }

    async fn create(
        &self,
        skel: HyperStarSkel,
        driver_skel: DriverSkel,
        ctx: DriverCtx,
    ) -> Result<Box<dyn Driver>, DriverErr> {
        let skel = HyperSkel::new(skel, driver_skel);
        Ok(Box::new(BundleDriver::new(skel, ctx)))
    }
}

pub struct BundleDriver

{
    skel: HyperSkel,
    ctx: DriverCtx,
}

#[handler]
impl BundleDriver

{
    pub fn new(skel: HyperSkel, ctx: DriverCtx) -> Self {
        Self { skel, ctx }
    }
}

#[async_trait]
impl Driver for BundleDriver

{
    fn kind(&self) -> Kind {
        Kind::Bundle
    }

    async fn item(&self, point: &Point) -> Result<ItemSphere, DriverErr> {
        Ok(ItemSphere::Handler(Box::new(Bundle)))
    }

    async fn handler(&self) -> Box<dyn DriverHandler> {
        Box::new(BundleDriverHandler::restore(
            self.skel.clone(),
            self.ctx.clone(),
        ))
    }
}

pub struct BundleDriverHandler

{
    skel: HyperSkel,
    ctx: DriverCtx,
}

impl BundleDriverHandler

{
    fn restore(skel: HyperSkel, ctx: DriverCtx) -> Self {
        Self { skel, ctx }
    }
}

impl DriverHandler for BundleDriverHandler {}

#[handler]
impl BundleDriverHandler

{
    fn store(&self) -> Result<ValueRepo<String>, SpaceErr> {
        let config = acid_store::store::DirectoryConfig {
            path: PathBuf::from(format!("{}artifacts", self.skel.star.data_dir())),
        };

        match OpenOptions::new()
            .mode(acid_store::repo::OpenMode::Create)
            .open(&config)
        {
            Ok(repo) => Ok(repo),
            Err(err) => return Err(SpaceErr::new(500u16, err.to_string())),
        }
    }
    #[route("Hyp<Assign>")]
    async fn assign(&self, ctx: InCtx<'_, HyperSubstance>) -> Result<(), DriverErr> {
        if let HyperSubstance::Assign(assign) = ctx.input {
            let state = match &assign.state {
                StateSrc::Substance(data) => data.clone(),
                StateSrc::None => {
                    return self
                        .skel
                        .driver
                        .logger
                        .result(Err("ArtifactBundle cannot be stateless".into()));
                }
            };
            if let Substance::Bin(zip) = (*state).clone() {
                let temp_dir = TempDir::new("zipcheck")?;
                let temp_path = temp_dir.path().clone();
                let file_path = temp_path.with_file_name("file.zip");
                let mut file = File::create(file_path.as_path())?;
                file.write_all(zip.as_slice())?;

                let file = File::open(file_path.as_path())?;
                let mut archive = zip::ZipArchive::new(file)?;
                let mut artifacts = vec![];
                for i in 0..archive.len() {
                    let file = archive.by_index(i).unwrap();
                    if !file.name().ends_with("/") {
                        artifacts.push(file.name().to_string())
                    }
                }

                {
                    let mut store = self.store()?;
                    let state = *state;
                    store.insert(assign.details.stub.point.to_string(), &state)?;
                    store.commit()?;
                }

                self.skel
                    .star
                    .registry
                    .assign_star(&assign.details.stub.point, &self.skel.star.point)
                    .await?;

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

                let mut point_and_kind_set: Vec<PointKind> =
                    point_and_kind_set.into_iter().collect();

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
                    let ctx = self.ctx.clone();
                    //                    tokio::spawn(async move {
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
                                    Err(err) => StateSrc::None,
                                }
                            }
                            _ => {
                                panic!("unexpected knd");
                            }
                        };

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

                        let wave: DirectedProto = create.into();
                        let pong: Wave<Pong> = ctx.transmitter.direct(wave).await.unwrap();
                    }
                    //   });
                }
            } else {
                return Err("ArtifactBundle Manager expected Bin payload".into());
            }

            Ok(())
        } else {
            Err(DriverErr::new("Bad Reqeust: expected Assign"))
        }
    }
}

pub struct Bundle;

#[handler]
impl Bundle {}

#[async_trait]
impl ItemHandler for Bundle

{
    async fn bind(&self) -> Result<ArtRef<BindConfig>, DriverErr> {
        Ok(BUNDLE_BIND_CONFIG.clone())
    }
}

pub struct ArtifactDriverFactory;

impl ArtifactDriverFactory {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl HyperDriverFactory for ArtifactDriverFactory

{
    fn kind(&self) -> KindSelector {
        KindSelector::from_base(BaseKind::Artifact)
    }

    async fn create(
        &self,
        skel: HyperStarSkel,
        driver_skel: DriverSkel,
        ctx: DriverCtx,
    ) -> Result<Box<dyn Driver>, DriverErr> {
        Ok(Box::new(ArtifactDriver::new(driver_skel, ctx)))
    }
}

pub struct ArtifactDriver

{
    skel: DriverSkel,
    ctx: DriverCtx,
}


impl ArtifactDriver

{
    pub fn new(skel: DriverSkel, ctx: DriverCtx) -> Self {
        Self { skel, ctx }
    }
}

#[handler]
impl ArtifactDriver

{
}

#[async_trait]
impl Driver for ArtifactDriver

{
    fn kind(&self) -> Kind {
        Kind::Artifact(ArtifactSubKind::Raw)
    }

    async fn item(&self, point: &Point) -> Result<ItemSphere, DriverErr> {
        let record = self.skel.locate(point).await?;

        let skel = ItemSkel::new(point.clone(), record.details.stub.kind, self.skel.clone());
        Ok(ItemSphere::Handler(Box::new(Artifact::restore(
            skel,
            (),
            (),
        ))))
    }

    async fn handler(&self) -> Box<dyn DriverHandler> {
        let skel = HyperSkel::new(self.skel.skel.clone(), self.skel.clone());
        Box::new(ArtifactDriverHandler::restore(skel))
    }
}

pub struct ArtifactDriverHandler

{
    skel: HyperSkel,
}

impl ArtifactDriverHandler

{
    fn restore(skel: HyperSkel) -> Self {
        Self { skel }
    }
}

impl DriverHandler for ArtifactDriverHandler  {}

#[handler]
impl ArtifactDriverHandler

{
    fn store(&self) -> Result<ValueRepo<String>, SpaceErr> {
        let config = acid_store::store::DirectoryConfig {
            path: PathBuf::from(format!("{}artifacts", self.skel.star.data_dir())),
        };

        match OpenOptions::new()
            .mode(acid_store::repo::OpenMode::Create)
            .open(&config)
        {
            Ok(repo) => Ok(repo),
            Err(err) => return Err(SpaceErr::new(500u16, err.to_string())),
        }
    }

    #[route("Hyp<Assign>")]
    async fn assign(&self, ctx: InCtx<'_, HyperSubstance>) -> Result<(), DriverErr> {
        if let HyperSubstance::Assign(assign) = ctx.input {
            if let Kind::Artifact(sub) = &assign.details.stub.kind {
                match sub {
                    ArtifactSubKind::Dir => {}
                    _ => {
                        let substance = assign.state.get_substance()?;
                        let mut store = self.skel.driver.logger.result(self.store())?;
                        store
                            .insert(assign.details.stub.point.to_string(), &substance)
                            .map_err(|e| SpaceErr::server_error(e.to_string()))?;
                        self.skel.driver.logger.result(
                            store
                                .commit()
                                .map_err(|e| SpaceErr::server_error(e.to_string())),
                        )?;
                    }
                }
                self.skel
                    .star
                    .registry
                    .assign_star(&assign.details.stub.point, &self.skel.star.point)
                    .await?;
            }
            Ok(())
        } else {
            Err(DriverErr::new("ArtifactDriver expected Assign"))
        }
    }
}

pub struct Artifact

{
    skel: ItemSkel,
}

#[handler]
impl Artifact

{
    fn store(&self) -> Result<ValueRepo<String>, SpaceErr> {
        let config = acid_store::store::DirectoryConfig {
            path: PathBuf::from(format!("{}artifacts", self.skel.data_dir())),
        };

        match OpenOptions::new()
            .mode(acid_store::repo::OpenMode::Create)
            .open(&config)
        {
            Ok(repo) => Ok(repo),
            Err(err) => return Err(SpaceErr::new(500u16, err.to_string())),
        }
    }

    #[route("Cmd<Read>")]
    pub async fn read(&self, _ctx: InCtx<'_, ()>) -> Result<Substance, DriverErr> {
        if let Kind::Artifact(ArtifactSubKind::Dir) = self.skel.kind {
            return Ok(Substance::Empty);
        }
        let store = self.store()?;

        let substance: Substance = store.get(&self.skel.point.to_string()).unwrap();
        Ok(substance)
    }

    #[route("Http<Get>")]
    pub async fn get(&self, _: InCtx<'_, ()>) -> Result<Substance, DriverErr> {
        if let Kind::Artifact(ArtifactSubKind::Dir) = self.skel.kind {
            return Ok(Substance::Empty);
        }
        let store = self.store()?;

        let substance: Substance = store.get(&self.skel.point.to_string()).unwrap();
        Ok(substance)
    }
}

impl Item for Artifact

{
    type Skel = ItemSkel;
    type Ctx = ();
    type State = ();

    fn restore(skel: Self::Skel, ctx: Self::Ctx, state: Self::State) -> Self {
        Self { skel }
    }
}

/*
#[async_trait]
impl DirectedHandler for Artifact where P: Cosmos {
    async fn handle(&self, ctx: RootInCtx) -> CoreBounce {
        println!("ARTIFACT HANDLE REQUEST : {}",ctx.wave.clone().to_wave().desc());

        let core = ReflectedCore {
            headers: Default::default(),
            status: Default::default(),
            body: Default::default()
        };
        CoreBounce::Reflected(core)
    }
}

 */

#[async_trait]
impl ItemHandler for Artifact

{
    async fn bind(&self) -> Result<ArtRef<BindConfig>, DriverErr> {
        Ok(ARTIFACT_BIND_CONFIG.clone())
    }
}


 */

#[cfg(test)]
pub mod tests {
    #[test]
    pub fn test() {}
}
