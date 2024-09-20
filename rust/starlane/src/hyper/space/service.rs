use crate::host::{ExeService,  HostEnv, OsEnv, Proc};
use crate::hyper::space::err::HyperErr;
use crate::store::FileStore;
use nom::AsBytes;
use starlane_space::command::common::{SetProperties, StateSrc};
use starlane_space::command::direct::create::{
    Create, KindTemplate, PointSegTemplate, PointTemplate, Strategy, Template,
};
use starlane_space::err::SpaceErr;
use starlane_space::hyper::{Assign, HyperSubstance};
use starlane_space::kind::{ArtifactSubKind, Kind};
use starlane_space::loc::{Surface, ToBaseKind};
use starlane_space::parse::Env;
use starlane_space::particle::PointKind;
use starlane_space::point::Point;
use starlane_space::substance::Substance;
use starlane_space::wave::core::hyp::HypMethod;
use starlane_space::wave::core::{CoreBounce, Method};
use starlane_space::wave::exchange::asynch::{DirectedHandler, InCtx, RootInCtx};
use starlane_space::wave::{
    Bounce, DirectedProto, DirectedWave, Pong, ReflectedWave, UltraWave, Wave,
};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::io::Read;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::process::Stdio;
use std::str::FromStr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command};
use tracing::instrument::WithSubscriber;
use std::hash::Hash;
use crate::err::StarErr;

pub struct Service {
    pub handler: Box<dyn DirectedHandler>
}

impl Service {
    pub fn new( handler: Box<dyn DirectedHandler> ) -> Self {
        Self { handler }
    }

    pub fn handler(&self) -> &dyn DirectedHandler {
        self.handler.as_ref()
    }
}

#[derive(Clone)]
pub enum Dialect {
    FileStore
}

impl Dialect {
    pub fn handler(&self, host: Host) -> Result<Box<dyn DirectedHandler>,StarErr> {
        match self {
            Dialect::FileStore => {
                let cli = host.executor().ok_or("Driver ")?;
                Ok(Box::new(FileStoreCliExecutor::new(cli)))
            }
        }

    }
}


#[derive(Clone)]
pub struct ServiceTemplate {
    pub exec: ExeInfo<String, HostEnv, Option<Vec<String>>>,
    pub dialect: Dialect
}

impl ServiceTemplate {
    pub fn new( exec: ExeInfo<String, HostEnv, Option<Vec<String>>>, dialect: Dialect ) -> Self {
        Self {
            exec,
            dialect
        }
    }

    pub fn create(&self) -> Result<Service, StarErr> {
        let host = self.exec.host.create(&self.exec.stub)?;
        let handler = self.dialect.handler(host)?;
        Ok(Service::new(handler))
    }
}

#[async_trait]
pub trait Executor
where
    Self::Err: HyperErr,
{
    type Args;
    type Err;
    type Spawn;
    async fn execute(&self, args: Self::Args) -> Self::Spawn;
}

impl FileStoreCliExecutor {
    async fn assign<'a>(&self, ctx: &'a InCtx<'_, Assign>) -> Result<(), <FileStoreCliExecutor as Executor>::Err> {
        async fn wait(
            mut child: OsProcess,
            line: String,
        ) -> Result<(), StarErr> {
            match child.wait().await?.success() {
                true => Ok(()),
                false => match child.stderr.as_mut() {
                    None => Err(SpaceErr::from(format!(
                        "host operation {} failed.  No error output encountered",
                        line
                    ))
                    .into()),
                    Some(err) => {
                        let mut message = String::new();
                        err.read_to_string(&mut message).await?;
                        Err(SpaceErr::from(format!(
                            "host operation {} failed.  StdErr: {}",
                            line, message
                        ))
                        .into())
                    }
                },
            }
        }

        let bin = match &ctx.state {
            StateSrc::Substance(data) => data.to_bin()?,
            StateSrc::None => Box::new(Substance::Empty).to_bin()?,
        };
        let line = format!("write {}", ctx.details.stub.point.to_path().display());
        let args = line.split_whitespace().map(|a|a.to_string()).collect::<Vec<String>>();
        let mut child = self.cli.execute(args).await?;
        let mut stdin = child.stdin.take().ok_or(SpaceErr::from(format!(
            "command {} could not write to StdIn",
            line
        )))?;
        stdin.write_all(bin.as_bytes()).await?;
        wait(child, line).await
    }
}

#[handler]
impl FileStoreCliExecutor {
    #[route("Hyp<Assign>")]
    async fn handle_assign(&self, ctx: InCtx<'_, HyperSubstance>) -> Result<(), StarErr> {
        if let HyperSubstance::Assign(assign) = ctx.input {
            let ctx = ctx.push_input_ref(assign);
            ctx.logger.result(self.assign(&ctx).await)
        } else {
            Err(StarErr::new("Bad Reqeust: expected Assign"))

        }
    }
}


pub struct OsProcess {
    child: Child,
}

impl Deref for OsProcess {
    type Target = Child;

    fn deref(&self) -> &Self::Target {
        &self.child
    }
}

impl DerefMut for OsProcess {
    fn deref_mut(&mut self) -> &mut Self::Target {
        & mut self.child
    }
}

impl OsProcess {
    pub fn new(child: Child) -> Self {
        Self { child }
    }
}

impl Proc for OsProcess {
    type StdOut = ChildStdout;
    type StdIn = ChildStdin;
    type StdErr = ChildStderr;

    fn stderr(&self) -> Option<&Self::StdErr> {
        self.child.stderr.as_ref()
    }

    fn stdout(&self) -> Option<&Self::StdOut> {
        self.child.stdout.as_ref()
    }

    fn stdin(&mut self) -> Option<&Self::StdIn> {
        self.child.stdin.as_ref()
    }
}

#[async_trait]
impl Executor for OsExeCli {
    type Args = Vec<String>;
    type Err = StarErr;
    type Spawn = Result<OsProcess, Self::Err>;

    async fn execute(&self, args: Self::Args) -> Self::Spawn {
        let mut command = Command::new(self.stub.loc.clone());
        command.envs(self.stub.env.env.clone());
        command.args(args);
        command.current_dir(self.stub.env.pwd.clone());
        command.env_clear();
        command.stdin(Stdio::piped()).output().await?;
        command.stdout(Stdio::piped()).output().await?;
        command.stderr(Stdio::piped()).output().await?;

        let child = command.spawn()?;
        Ok(OsProcess::new(child))
    }
}


pub trait CliExecutor:Executor<Args=Vec<String>,Spawn=Result<OsProcess,StarErr>,Err=StarErr> {

}



#[derive(Clone)]
pub struct OsExeCli {
    pub stub: OsExeStub,
}

impl OsExeCli {
    pub fn new<I>(info: I) -> Self where I:Into<OsExeStub>
    {
        let info = info.into();
        Self { stub: info }
    }
}

impl CliExecutor for OsExeCli{}

#[derive(DirectedHandler)]
pub struct FileStoreCliExecutor {
    pub cli: Box<dyn CliExecutor>,
}

impl FileStoreCliExecutor {
    pub fn new( cli: Box<dyn CliExecutor>) -> Self {
        Self { cli }
    }
}

#[async_trait]
impl Executor for FileStoreCliExecutor {
    type Args = RootInCtx;
    type Err = StarErr;
    type Spawn = CoreBounce;

    async fn execute(&self, args: Self::Args) -> Self::Spawn {
        DirectedHandler::handle(self, args).await
    }
}


#[cfg(test)]
pub mod tests {
    use std::env;
    use std::path::{absolute, PathBuf};
    use crate::host::{HostEnvBuilder, HostEnv};
    use crate::hyper::space::service::{Dialect, ExeInfo, HostApi, OsExeInfo, OsExeStub, OsExeStubArgs};
    use crate::hyper::space::service::ServiceTemplate;

    #[tokio::test]
    pub async fn test() {
        let mut builder = HostEnv::builder();
        builder.pwd( absolute( env::current_dir().unwrap()).unwrap().to_str().unwrap().to_string() );
        builder.env("DATA_DIR", "./");
        let env = builder.build();
        let path = PathBuf::from("./runme.sh");
        let args: Option<Vec<String>> = Option::None;
        let stub = OsExeStub::new(path.to_string(), env, args );
        let exec = ExeInfo::new(HostApi::Cli, stub );
        let template = ServiceTemplate::new( exec, Dialect::FileStore );
        let service = template.create().unwrap();
    }


}

#[derive(Clone, Hash, Eq, PartialEq)]
pub enum HostApi {
    Cli(HostKind)
}

#[derive(Clone, Hash, Eq, PartialEq)]
pub enum HostKind{
   Os
}


pub enum Host {
  Cli(CliHost)
}

impl Host {




    pub fn is_cli(&self) -> bool {
        match self {
            Host::Cli(_) => true
        }
    }

    pub fn executor(&self) -> Option<Box<dyn Executor<Spawn=OsProcess,Err=StarErr, Args=Vec<String>>>>{
        match self {
            Host::Cli(exec) => {
                Some(Box::new(exec.clone()))
            }
        }

    }
}

pub enum CliHost {
    Os(OsExeCli)
}

impl CliHost {
    pub fn executor(&self) -> &OsExeCli {
        match self {
            CliHost::Os(exec) => {
                exec
            }
        }
    }
}

impl Host {

}


impl HostApi {

    pub async fn create<S>(&self, stub: S) -> Result<Host,StarErr> where S: Into<OsExeStub>
    {
        match self {
            HostApi::Cli(HostKind::Os) => {
                let exe = OsExeCli::new(stub);
                let host = CliHost::Os(exe);
                let host = Host::Cli(host);
                Ok(host)
            }
        }

    }
}

#[derive(Clone, Hash, Eq, PartialEq)]
pub struct ExeStub <L,E,A> where E: Clone+Hash+Eq+PartialEq, L: Clone+Hash+Eq+PartialEq, A: Clone+Hash+Eq+PartialEq{
    pub loc: L,
    pub env: E,
    args: A,
}

impl <L,E,A> ExeStub <L,E,A> where E: Clone+Hash+Eq+PartialEq, L: Clone+Hash+Eq+PartialEq, A: Clone+Hash+Eq+PartialEq{
    pub fn new(loc: L, env: E, args: A) -> Self {
        Self { loc, env, args }
    }
}

impl <E> Into<ExeStub<PathBuf, OsEnv,()>> for ExeStub<String,E,()>  where E: Into<HostEnv>+Clone+Hash+Eq+PartialEq {

    fn into(self) -> ExeStub<PathBuf, HostEnv, ()> {
        ExeStub {
            loc: self.loc.into(),
            env: self.env.into(),
            args: ()
        }
    }
}

pub type OsExeInfo = ExeInfo<PathBuf, OsEnv,()>;
pub type OsExeStub = ExeStub<PathBuf, OsEnv,()>;
pub type OsExeStubArgs = ExeStub<PathBuf, HostEnv,Vec<String>>;

#[derive(Clone, Hash, Eq, PartialEq)]
pub struct ExeInfo<L,E,A> where E: Clone+Hash+Eq+PartialEq, L: Clone+Hash+Eq+PartialEq, A: Clone+Hash+Eq+PartialEq{
    pub host: HostApi,
    pub stub: ExeStub<L,E,A>,
}

impl <L,E,A> ExeInfo<L,E,A> where E: Clone+Hash+Eq+PartialEq, L: Clone+Hash+Eq+PartialEq, A: Clone+Hash+Eq+PartialEq{
    pub fn new(host: HostApi, stub: ExeStub<L,E,A>) -> Self {
        Self { host, stub }
    }
}

#[cfg(test)]
pub mod test {}