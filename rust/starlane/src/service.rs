use crate::err::StarErr;
use crate::hyperspace::err::HyperErr;
use itertools::Itertools;
use nom::AsBytes;
use starlane_space as starlane;
use starlane::space::asynch::state_relay;
use starlane::space::kind::Kind;
use starlane::space::loc::{Surface, ToBaseKind};
use starlane::space::log::PointLogger;
use starlane::space::particle::Status;
use starlane::space::point::Point;
use starlane::space::util::{IdSelector, MatchSelector, OptSelector, ValueMatcher};
use starlane::space::wave::exchange::asynch::{
    DirectedHandler, DirectedHandlerShell, Router,
};
use starlane::space::wave::{Bounce, DirectedWave, ReflectedWave};
use std::collections::HashSet;
use std::future::Future;
use std::hash::Hash;
use std::io::Read;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use strum_macros::{EnumIter, EnumString};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::watch;
use tracing::instrument::WithSubscriber;
use starlane::space::selector::KindSelector;
use crate::executor::cli::HostEnv;
use crate::host::ExeInfo;

#[derive(Debug, Clone, Hash, Eq, PartialEq,EnumString)]
pub enum ServiceKind {
    FileStore
}


/// at this time the ServiceKindSelector
/// is an exact match for the ServiceKind
pub type ServiceKindSelector = ServiceKind;


pub struct ServiceCreationSelector {
    pub selector: ServiceSelector,
    pub ctx: ServiceCtx,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct ServiceKey {
    pub name: String,
    pub kind: ServiceKind,
    pub share: ServiceShare,
}

pub struct ServiceSelector {
    pub name: IdSelector<String>,

    pub kind: MatchSelector<KindSelector, Kind>,
    pub share: IdSelector<ServiceShare>,
    pub star: OptSelector<IdSelector<Point>>,
    pub driver: OptSelector<IdSelector<Point>>,
    pub particle: OptSelector<IdSelector<Point>>,
}

impl PartialEq<ServiceKey> for ServiceSelector {
    fn eq(&self, key: &ServiceKey) -> bool {
        //self.name == key.name && self.kind == key.kind && self.share == key.share
        todo!()
    }
}
impl PartialEq<ServiceTemplate> for ServiceSelector {
    fn eq(&self, key: &ServiceTemplate) -> bool {
        todo!()
//        self.name == key.name && self.kind == key.kind
    }
}

/*
pub struct ServicePool {
    core: RwLock<ServicePoolCore>
}

impl ServicePool {


    async fn create( & self, template: &ServiceTemplate, pwd: PathBuf, mount: Point ) -> Result<ServiceStub,StarErr> {
        let mut info = template.exec.clone();
        info.stub.env.pwd = self.ctx.data_dir.join(mount.to_path()).to_str().unwrap().to_string();
        let host = info.create_host()?;
        let handler = template.dialect.handler(host)?;

        Ok(Arc::new(ServiceHandler::new(handler)))
    }
}



pub struct ServicePoolCore
{
    ctx: ServiceCtx,
    templates: Templates<ServiceTemplate>,
    services: HashMap<ServiceKey,ServiceStub>,
}

impl ServicePoolCore {

    pub fn create(&mut self, create: &ServiceCreationSelector) -> Result<Option<ServiceStub>,StarErr> {
        match self.select_from_template(&create.selector) {
            None => Ok(None),
            Some(template) => {
                let core = >ServiceCore::create( create.ctx.clone(), template )?;
                Ok(Some(ServiceRunner::new(core)))
            }
        }

    }

    pub fn select_from_template(&mut self, selector: &ServiceSelector ) -> Option<ServiceTemplate> {
        self.templates.select_one(selector).cloned()
    }
}

 */

pub trait Service
where
    Self::Handler: DirectedHandler,
{
    type Handler;

    fn handler(&self) -> &Self::Handler;
}

pub struct ServiceHandler<D>
where
    D: DirectedHandler,
{
    handler: D,
}

impl<D> ServiceHandler<D>
where
    D: DirectedHandler,
{
    pub fn new(handler: D) -> Self {
        Self { handler }
    }
}

impl<D> Service for ServiceHandler<D>
where
    D: DirectedHandler,
{
    type Handler = D;

    fn handler(&self) -> &Self::Handler {
        &self.handler
    }
}

#[derive(Debug, Clone, Hash, Eq, PartialEq, EnumIter)]
pub enum ServiceShare {
    /// one service for everyone
    Singleton,
    /// one of this Service per star
    Star,
    /// one Service Per driver
    Driver,
    /// unique service per particle
    Particle
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum ServiceAgent {
    Singleton,
    Star(Point),
    Driver {
        star: Point,
        driver: Point,
    },
    Particle {
        star: Point,
        driver: Point,
        particle: Point,
    },
}

#[derive(Debug, Clone, Eq, PartialEq, EnumString)]
pub enum ServiceShareSelector {
    Any,
    Set(HashSet<ServiceShare>),
}

impl PartialEq<ServiceShare> for ServiceShareSelector {
    fn eq(&self, other: &ServiceShare) -> bool {
        match &self {
            ServiceShareSelector::Any => true,
            ServiceShareSelector::Set(set) => set.contains(other),
        }
    }
}

impl ServiceShareSelector {
    pub fn new() -> Self {
        Self::Any
    }

    pub fn or(self, share: ServiceShare) -> Self {
        match self {
            ServiceShareSelector::Any => Self::Set(HashSet::from([share])),
            ServiceShareSelector::Set(mut set) => {
                set.insert(share);
                ServiceShareSelector::Set(set)
            }
        }
    }
}

impl Default for ServiceShareSelector {
    fn default() -> Self {
        Self::Any
    }
}

#[derive(Clone)]
pub struct ServiceTemplate {
    pub name: String,
    pub kind: ServiceKind,
    pub share: ServiceShare,
    pub exec: ExeInfo<String, HostEnv, Option<Vec<String>>>,
}

impl ServiceTemplate {
    /*
    pub fn create(&self, ctx: ServiceCtx, mount: &Point) -> Result<Arc<dyn Service<Handler=Box<dyn DirectedHandler>>>, StarErr> {
        let mut exec = self.exec.clone();
        exec.stub.env.pwd = ctx.data_dir.join(mount.to_path()).to_str().unwrap().to_string();
        let host = self.exec.host.create(exec.stub.clone())?;
        let handler = self.dialect.handler(host)?;

        Ok(Arc::new(ServiceHandler::new(handler)))
    }

     */
}

impl Into<ServiceKey> for ServiceTemplate {
    fn into(self) -> ServiceKey {
        ServiceKey {
            name: self.name.clone(),
            kind: self.kind.clone(),
            share: self.share.clone(),
        }
    }
}

#[derive(Clone)]
pub struct ServiceCtx {
    pub surface: Surface,
    pub data_dir: PathBuf,
    pub router: Arc<dyn Router>,
    pub logger: PointLogger,
}

impl ServiceCtx {
    pub fn new(
        surface: Surface,
        data_dir: PathBuf,
        router: Arc<dyn Router>,
        logger: PointLogger,
    ) -> Self {
        Self {
            surface,
            data_dir,
            router,
            logger,
        }
    }
}

pub struct ServiceCall {
    pub from: Point,
    pub tx: tokio::sync::oneshot::Sender<Bounce<ReflectedWave>>,
    pub command: ServiceCommand,
}

pub enum ServiceCommand {
    DirectedWave(DirectedWave),
}

#[derive(Clone)]
pub struct ServiceStub {
    template: ServiceTemplate,
    call_tx: tokio::sync::mpsc::Sender<ServiceCall>,
    status_rx: watch::Receiver<Status>,
}

pub struct ServiceRunner<D>
where
    D: DirectedHandler + 'static,
{
    call_rx: tokio::sync::mpsc::Receiver<ServiceCall>,
    status_tx: tokio::sync::mpsc::Sender<Status>,
    core: ServiceCore<D>,
}

impl<D> ServiceRunner<D>
where
    D: DirectedHandler,
{
    fn new(core: ServiceCore<D>) -> ServiceStub {
        let (call_tx, call_rx) = tokio::sync::mpsc::channel(1024);
        let (status_tx, status_rx) = state_relay(Status::Pending);
        let template = core.template.clone();
        let rtn = ServiceStub {
            call_tx,
            status_rx,
            template,
        };

        let runner = Self {
            call_rx,
            status_tx,
            core,
        };

        tokio::spawn(async move { runner.launch().await });

        rtn
    }

    async fn launch(mut self) {
        let status_tx = self.status_tx.clone();
        let logger = self.core.ctx.logger.clone();
        match logger.result(self.run().await) {
            Ok(status) => {
                status_tx.send(status);
            }
            Err(_) => {
                status_tx.send(Status::Panic);
            }
        }
    }

    async fn run(mut self) -> Result<Status, StarErr> {
        self.status_tx.send(Status::Ready);

        while let Some(call) = self.call_rx.recv().await {
            match call.command {
                ServiceCommand::DirectedWave(wave) => {
                    self.core.handler.handle(wave).await;
                }
            }
        }

        Ok(Status::Done)
    }
}

struct ServiceCore<D>
where
    D: DirectedHandler,
{
    ctx: ServiceCtx,
    template: ServiceTemplate,
    handler: DirectedHandlerShell<D>,
}

impl<D> ServiceCore<D>
where
    D: DirectedHandler,
{
    /*
    pub fn create(ctx: ServiceCtx, template: ServiceTemplate ) -> Result<Self,StarErr>{
        let host = template.host.create( template.exec.stub.clone() )?;
        let exchanger= Exchanger::new(ctx.surface.clone(), Timeouts::default(), ctx.logger.clone() );
        let mut builder = ProtoTransmitterBuilder::new(ctx.router.clone(), exchanger);
        builder.from = SetStrategy::Override(ctx.surface.clone());
        let handler = template.dialect.handler(host)?;
        let handler = DirectedHandlerShell::new( handler, builder, ctx.surface.clone(), ctx.logger.logger.clone());
        Ok(Self {
            ctx,
            template,
            handler
        })
    }

     */

    /*
    pub fn handler( & self ) -> D {
        self.handler.clone()
    }

     */
}

#[cfg(test)]
pub mod tests {
    use crate::host::{ExeInfo, ExeStub };

    use nom::AsBytes;
    use starlane::space::command::common::StateSrc;
    use starlane::space::hyper::{Assign, AssignmentKind, HyperSubstance};
    use starlane::space::kind::{FileSubKind, Kind};
    use starlane::space::loc::ToSurface;
    use starlane::space::log::RootLogger;
    use starlane::space::particle::{Details, Status, Stub};
    use starlane::space::point::Point;
    use starlane::space::substance::Substance;
    use starlane::space::wave::core::hyp::HypMethod;
    use starlane::space::wave::exchange::asynch::{DirectedHandler, Exchanger, ProtoTransmitterBuilder, RootInCtx, TxRouter};
    use starlane::space::wave::{DirectedKind, DirectedProto};
    use std::path::{absolute, PathBuf};
    use std::str::FromStr;
    use std::sync::Arc;
    use std::{env, io};
    use tokio::fs;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use wasmer::IntoBytes;
    use starlane::space::wave::core::Method;
    use crate::err::StarErr;
    use crate::executor::Executor;
    use crate::executor::cli::{CliExecutor, HostEnv};
    use crate::executor::cli::os::{OsEnv, OsExeInfo, OsStub};
    use crate::executor::dialect::filestore::{FileStoreCli, FileStoreCommand, FileStore, FileStoreIn, FileStoreOut};
    use crate::executor::dialect::{HostDialect, HostRunner};

    fn filestore() -> Box<FileStore> {
        if std::fs::exists("./tmp").unwrap() {
            std::fs::remove_dir_all("./tmp").unwrap();
        }
        let mut builder = HostEnv::builder();
        builder.pwd(
            absolute(env::current_dir().unwrap())
                .unwrap()
                .to_str()
                .unwrap()
                .to_string(),
        );
        println!("{}", env::current_dir().unwrap().to_str().unwrap());
        builder.env(
            "FILE_STORE_ROOT",
            format!("{}/tmp", env::current_dir().unwrap().to_str().unwrap()),
        );
        let env = builder.build();
        let path = "../target/debug/starlane-cli-filestore-service".to_string();
        let args: Option<Vec<String>> = Option::None;
        let stub: OsStub = ExeStub::new(path.into(), env, ());
//        let info = ExeInfo::new(HostDialect::Cli(HostRunner::Os), stub);

        let info = OsExeInfo::new( HostDialect::Cli(HostRunner::Os), stub);

        let executor: Box<FileStore> = info.create().unwrap();

        executor
    }

    pub async fn create_dialect_handler() -> Result<Box<dyn DirectedHandler>,StarErr>{
        let logger = RootLogger::default();
        let host = filestore();
//        let filestore = DialectKind::FileStore.handler(host).unwrap();
        todo!();

        let fae = Point::from_str("fae").unwrap();
        let less = Point::from_str("less").unwrap();

        let to = fae.clone().to_surface();
        let logger = logger.point(to.point.clone());
        let (tx, rx) = tokio::sync::mpsc::channel(1024);
        let router = Arc::new(TxRouter::new(tx));

        let exchanger = Exchanger::new(to.clone(), Default::default(), logger.clone());
        let mut tx_builder = ProtoTransmitterBuilder::new(router, exchanger);
        let transmitter = tx_builder.build();

//        let mut ctx = RootInCtx::new(wave, to, logger.span(), transmitter);

//        filestore.handle(ctx).await;
//        Ok(filestore)
    }


    fn wave( method: Method, body: Substance) -> RootInCtx {
        let logger = RootLogger::default();
        let fae = Point::from_str("fae").unwrap();
        let less = Point::from_str("less").unwrap();

        let mut wave = DirectedProto::kind(&DirectedKind::Ping);
        wave.method(method);
        let fae = Point::from_str("fae").unwrap();
        let less = Point::from_str("less").unwrap();
        wave.to(fae.clone().to_surface());
        wave.from(less.clone().to_surface());
        wave.body(body);

        let wave = wave.build().unwrap();
        let to = Point::central().to_surface();
        let logger = logger.point(to.point.clone());
        let (tx, rx) = tokio::sync::mpsc::channel(1024);
        let router = Arc::new(TxRouter::new(tx));

        let exchanger = Exchanger::new(to.clone(), Default::default(), logger.clone());
        let mut tx_builder = ProtoTransmitterBuilder::new(router, exchanger);

        let transmitter = tx_builder.build();

        let mut ctx = RootInCtx::new(wave, to, logger.span(), transmitter);

        ctx
    }
    #[tokio::test]
    pub async fn test_dialect() {

        let filestore = create_dialect_handler().await.unwrap();

        let fae = Point::from_str("somefile.txt").unwrap();
        let less = Point::from_str("less").unwrap();

        let ctx = wave(Method::Hyp(HypMethod::Init),Substance::Hyper(HyperSubstance::Empty));

        filestore.handle(ctx).await;

        assert!(PathBuf::from_str("./tmp").unwrap().exists());



        let assign = Assign::new(
            AssignmentKind::Create,
            Details::new(
                Stub {
                    point: fae,
                    kind: Kind::File(FileSubKind::File),
                    status: Status::Unknown,
                },
                Default::default(),
            ),
            StateSrc::Substance(Box::new(Substance::Text("helllo everyone".to_string()))),
        );

        let body = Substance::Hyper(HyperSubstance::Assign(assign));

        let ctx = wave(Method::Hyp(HypMethod::Assign),body );

        filestore.handle(ctx).await;
    }

    /*
    #[tokio::test]
    pub async fn test_dialect_old() {
        let logger = RootLogger::default();
        let host = filestore();
        let filestore = Dialect::FileStore.handler(host).unwrap();
        let mut wave = DirectedProto::kind(&DirectedKind::Ping);
        wave.method(HypMethod::Assign);
        let fae = Point::from_str("fae").unwrap();
        let less = Point::from_str("less").unwrap();
        wave.to(fae.clone().to_surface());
        wave.from(less.clone().to_surface());

        let assign = Assign::new(
            AssignmentKind::Create,
            Details::new(
                Stub {
                    point: fae,
                    kind: Kind::File(FileSubKind::File),
                    status: Status::Unknown,
                },
                Default::default(),
            ),
            StateSrc::Substance(Box::new(Substance::Text("helllo everyone".to_string()))),
        );

        wave.body(Substance::Hyper(HyperSubstance::Assign(assign)));
        let wave = wave.build().unwrap();
        let to = Point::central().to_surface();
        let logger = logger.point(to.point.clone());
        let (tx, rx) = tokio::sync::mpsc::channel(1024);
        let router = Arc::new(TxRouter::new(tx));

        let exchanger = Exchanger::new(to.clone(), Default::default(), logger.clone());
        let mut tx_builder = ProtoTransmitterBuilder::new(router, exchanger);

        let transmitter = tx_builder.build();

        let mut ctx = RootInCtx::new(wave, to, logger.span(), transmitter);

        filestore.handle(ctx).await;
    }

     */

    /*
    #[tokio::test]
    pub async fn test_cli_primitive() {
        if let Host::Cli(CliHost::Os(exe)) = filestore() {
            let args = FileStoreCli::new(FileStoreCommand::Init);
            let mut child = exe.execute(args).await.unwrap();
            //           let mut stdout = child.stdout.take().unwrap();
            drop(child.stdout.take().unwrap());

            let mut output = child.wait().await.unwrap();
/*
            tokio::io::copy(&mut output.stdout.as_bytes(), &mut tokio::io::stdout())
                .await
                .unwrap();
            tokio::io::copy(&mut output.stderr.as_bytes(), &mut tokio::io::stderr())
                .await
                .unwrap();

 */
        } else {
            assert!(false)
        }
    }

     */

    #[tokio::test]
    pub async fn test_os_cli_host() {
        let executor = filestore();

        if let io::Result::Ok(true) = fs::try_exists("./tmp").await {
            fs::remove_dir_all("./tmp").await.unwrap();
        }

        // init
        {
            let init = FileStoreIn::Init;
            executor
                .execute(init)
                .await.unwrap();
        }

        let path = PathBuf::from("tmp");
        assert!(path.exists());
        assert!(path.is_dir());

        {

            let args = FileStoreIn::Mkdir {path: "blah".into()};

            let mut child = executor
                .execute(args)
                .await
                .unwrap();
        }

        let path = PathBuf::from("tmp/blah");
        assert!(path.exists());
        assert!(path.is_dir());

        let content = "HEllo from me";

        {

            let args = FileStoreIn::Write{path: "blah/somefile.txt".into(), state: content.clone().into()};
            let mut child = executor
                .execute(args)
                .await
                .unwrap();

        }

        let path = PathBuf::from("tmp/blah/somefile.txt");
        assert!(path.exists());
        assert!(path.is_file());

        {
            let args = FileStoreIn::Read{ path: "blah/somefile.txt".into() };
            let mut child = executor
                .execute(args)
                .await
                .unwrap();
            if let FileStoreOut::Read(bin) = child {
                let read = String::from_utf8(bin).unwrap();
                println!("content: {}", read);
                assert_eq!(content, read);
            } else {
                assert!(false);
            }

        }


    }
}
