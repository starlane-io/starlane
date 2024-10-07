use std::env;
use crate::executor::{ExeConf, Executor};
use itertools::Itertools;
use nom::AsBytes;
use starlane::space::loc::ToBaseKind;
use starlane::space::util::{IdSelector, MatchSelector, OptSelector, RegexMatcher, ValueMatcher};
use starlane::space::wave::exchange::asynch::{
    DirectedHandler, Router,
};
use starlane_space as starlane;
use std::future::Future;
use std::hash::Hash;
use std::io::Read;
use std::ops::{Deref, DerefMut};
use std::path::{absolute, PathBuf};
use std::str::FromStr;
use strum_macros::EnumString;
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use starlane::space::err::SpaceErr;
use starlane::space::kind::Kind;
use starlane::space::particle::Status;
use starlane::space::point::Point;
use starlane::space::selector::KindSelector;
use crate::env::STARLANE_DATA_DIR;
use crate::executor::cli::HostEnv;
use crate::executor::cli::os::CliOsExecutor;
use crate::executor::dialect::filestore::{FileStore, FileStoreErr, FILE_STORE_ROOT};
use crate::host::{ExeStub, Host, HostCli};
use crate::hyperspace::machine::MachineErr;

pub type FileStoreService = Service<FileStore>;

impl FileStoreService {
    pub async fn sub_root( &self, sub_root: PathBuf) -> Result<FileStoreService, ServiceErr> {
        let runner = self.runner.sub_root(sub_root).await?;
        Ok(FileStoreService {
            template: self.template.clone(),
            runner
        })
    }
}


pub struct ServiceCall<I,O> {
    pub input: I,
    pub output: oneshot::Sender<Result<O, ServiceErr>>,
}

#[derive(Clone)]
pub struct ServiceStub<I,O> {
    tx: tokio::sync::mpsc::Sender<ServiceCall<I,O>>,
    status: tokio::sync::watch::Receiver<Status>,
}

pub struct Service<R> {
    pub template: ServiceTemplate,
    runner: R
}

impl Service<ServiceRunner>  {
    pub fn new( template: ServiceTemplate )  -> Service<ServiceRunner>{
        let runner = template.config.clone();
        Self {
            template,
            runner
        }
    }

    pub fn filestore(  self  ) -> Result<FileStoreService, ServiceErr> {
       Ok(FileStoreService{
           template: self.template,
           runner: self.runner.filestore()?
       })
    }
}



impl <R> Deref for Service<R> {
    type Target = R;

    fn deref(&self) -> &Self::Target {
        & self.runner
    }
}


#[derive(Clone)]
pub enum ServiceRunner {
    Exe(ExeConf)
}



impl ServiceRunner {
    pub fn filestore( & self  ) -> Result<FileStore, ServiceErr> {
        match self {
            ServiceRunner::Exe(exe) => {
                exe.create()
            }
        }
    }
}

#[derive(Hash,Clone,Eq,PartialEq,Debug,EnumString,strum_macros::Display)]
pub enum ServiceKind {
    FileStore
}

impl Into<Service<ServiceRunner>> for ServiceTemplate {
    fn into(self) -> Service<ServiceRunner> {
        let runner = self.config.clone();
        Service {
            template: self,
            runner
        }
    }
}


impl TryInto<Service<FileStore>> for Service<ServiceRunner>{
    type Error = ServiceErr;

    fn try_into(self) -> Result<Service<FileStore>, Self::Error> {
        let filestore = self.runner.filestore()?;
        Ok(Service{
            template: self.template,
            runner: filestore,
        })
    }
}

#[derive(Clone,Debug)]
pub struct ServiceSelector {
    pub name: IdSelector<String>,
    pub kind: ServiceKind,
    pub driver: Option<Kind>
}

impl ToString for ServiceSelector {
    fn to_string(&self) -> String {
        match &self.driver {
            None => {
                format!("{}<*:{}>",self.name.to_string(),self.kind.to_string())
            }
            Some(kind) => {
                format!("{}<{}:{}>",self.name.to_string(),kind.to_string(),self.kind.to_string())
            }
        }
    }
}

#[derive(Clone,Eq,PartialEq,Debug,Hash)]
pub enum ServiceScopeKind {
    Global,
    Point
}


#[derive(Clone,Eq,PartialEq,Debug,Hash)]
pub enum ServiceScope {
    Global,
    Point(Point)
}

#[derive(Clone)]
pub struct ServiceTemplate {
    pub name: String,
    pub kind: ServiceKind,
    // matches drivers that are allowed to use this Service
    pub driver: OptSelector<KindSelector>,
    pub config: ServiceConf
}

impl PartialEq<ServiceTemplate> for ServiceSelector{
    fn eq(&self, other: &ServiceTemplate) -> bool {
        self.name == other.name &&
            self.kind == other.kind &&
            other.driver == self.driver
    }
}


// at this time, Conf and Runner do not differ
pub type ServiceConf = ServiceRunner;


/*

#[derive(Clone)]
pub struct ServiceStub<C> {
    pub template: ServiceTemplate,
    pub call_tx: tokio::sync::mpsc::Sender<C>,
    pub status_rx: watch::Receiver<Status>,
}

pub struct ServiceRunner<Core,Call> where Core: ServiceCore<Call>
{
    ctx: ServiceCtx,
    call_rx: tokio::sync::mpsc::Receiver<Call>,
    status_tx: tokio::sync::mpsc::Sender<Status>,
    core: Core,
}

impl<Core,Call> ServiceRunner<Core,Call>
where Core: ServiceCore<Call>
{
    fn new(ctx: ServiceCtx, core: Core) -> ServiceStub<Call> {
        let (call_tx, call_rx) = tokio::sync::mpsc::channel(1024);
        let (status_tx, status_rx) = state_relay(Status::Pending);
        let template = ctx.template.clone();
        let rtn = ServiceStub {
            call_tx,
            status_rx,
            template,
        };

        let runner = Self {
            ctx,
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
            self.core.handle(wave).await;
        }

        Ok(Status::Done)
    }
}

pub trait ServiceCore<C>
{
    fn call(&self, ctx: &ServiceCtx, call: C );
}




 */

pub fn service_conf() -> ServiceConf{

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
        FILE_STORE_ROOT,
        STARLANE_DATA_DIR.to_string(),
    );
    let env = builder.build();
    let path = "../target/debug/starlane-cli-filestore-service".to_string();
    let args: Option<Vec<String>> = Option::None;

    let stub = ExeStub::new(path.into(), env);

    ServiceConf::Exe(ExeConf::Host(Host::Cli(HostCli::Os(stub))))

}

#[cfg(test)]
pub mod tests {
    use crate::host::{ExeStub, Host};

    use crate::executor::cli::HostEnv;
    use crate::executor::dialect::filestore::{FileStore, FileStoreIn, FileStoreOut};
    use crate::host::HostCli;
    use crate::executor::{ExeConf, Executor};
    use std::path::{absolute, PathBuf};
    use std::{env, io};
    use tokio::fs;
    use starlane::space::kind::{BaseKind, Kind};
    use starlane::space::selector::KindSelector;
    use starlane::space::util::OptSelector;
    use crate::service::{service_conf, Service, ServiceConf, ServiceErr, ServiceKind, ServiceTemplate};

    fn filestore() -> FileStore {
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
        let stub = ExeStub::new(path.into(), env);
        //        let info = ExeInfo::new(HostDialect::Cli(HostRunner::Os), stub);

        let info = ExeConf::Host(Host::Cli(HostCli::Os(stub.clone())));

         info.create().unwrap()

    }

    pub async fn filestore_from_service()  -> Result<Service<FileStore>, ServiceErr> {

        let config = service_conf();

       let template = ServiceTemplate {
           name: "some-filestore".to_string(),
           kind: ServiceKind::FileStore,
           driver: OptSelector::Selector(KindSelector::from_base(BaseKind::Repo)),
           config
       };

       let service = Service::new(template);

       Ok(service.try_into()?)

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
    pub async fn test_filestore() {
        let executor= filestore_from_service().await.unwrap();


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
            let args = FileStoreIn::Mkdir { path: "blah".into() };

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
            let args = FileStoreIn::Write { path: "blah/somefile.txt".into(), state: content.clone().into() };
            let mut child = executor
                .execute(args)
                .await
                .unwrap();
        }

        let path = PathBuf::from("tmp/blah/somefile.txt");
        assert!(path.exists());
        assert!(path.is_file());

        {
            let args = FileStoreIn::Read { path: "blah/somefile.txt".into() };
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


#[derive(Debug,Error,Clone)]
pub enum ServiceErr {
    #[error(transparent)]
    MachineErr(#[from] MachineErr),
    #[error(transparent)]
    FileStoreErr(#[from] FileStoreErr),
    #[error(transparent)]
    SpaceErr(#[from] SpaceErr),
    #[error("no template available that matches ServiceSelector: '{0}' (name<DriverKind:ServiceKind>)")]
    NoTemplate(ServiceSelector),
    #[error("call not processed")]
    CallRecvErr(#[from] tokio::sync::oneshot::error::RecvError),
}