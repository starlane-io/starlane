use crate::host::err::HostErr;
use crate::hyperspace::err::HyperErr;
use itertools::Itertools;
use starlane::space::wave::exchange::asynch::{DirectedHandler, InCtx, RootInCtx};
use std::collections::HashMap;
use std::env;
use std::fmt::Write;
use std::hash::{Hash, Hasher};
use std::io::Read;
use std::path::PathBuf;
use virtual_fs::FileSystem;
use tokio_print::aprintln;
use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command};
use std::process::Stdio;
use starlane::space::command::common::StateSrc;
use starlane::space::err::SpaceErr;
use starlane::space::hyper::{Assign, HyperSubstance};
use starlane::space::substance::Substance;
use std::ops::{Deref, DerefMut};
use clap::CommandFactory;
use nom::AsBytes;
use starlane::space::wave::core::CoreBounce;
use tokio::io::AsyncWriteExt;
use crate::dialect::cli::filestore::{Cli, Commands};
use crate::err::StarErr;
use crate::executor::Executor;

pub mod err;
pub mod ext;

//pub mod wasm;

#[derive(Clone, Eq, PartialEq, Hash)]
pub struct ExtKey<B>
where
    B: Hash + Eq + PartialEq,
{
    bin: B,
    env: HostEnv,
}

impl<B> ExtKey<B>
where
    B: Clone + Hash + Eq + PartialEq,
{
    pub fn new(bin: B, env: HostEnv) -> Self {
        Self { bin, env }
    }
}



pub type OsStub = ExeInfo<PathBuf, HostEnv, ()>;

pub trait Proc {
    type StdOut;
    type StdErr;
    type StdIn;
    fn stderr(&self) -> Option<&Self::StdErr>;
    fn stdout(&self) -> Option<&Self::StdOut>;

    fn stdin(&mut self) -> Option<&Self::StdIn>;
}

pub type OsEnv = HostEnv;
pub type WasmEnv = HostEnv;

#[derive(Clone, Eq, PartialEq)]
pub struct HostEnv {
    pub pwd: String,
    pub env: HashMap<String, String>,
}

impl HostEnv {
    pub fn builder() -> HostEnvBuilder {
        HostEnvBuilder::default()
    }
}

impl Hash for HostEnv {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_str(self.pwd.as_str());
        for key in self.env.keys().sorted() {
            state.write_str(key.as_str());
            state.write_str(self.env.get(key).unwrap());
        }
    }
}

impl Default for HostEnv {
    fn default() -> Self {
        Self {
            pwd: env::current_dir().unwrap().to_str().unwrap().to_string(),
            env: HashMap::default(),
        }
    }
}

#[derive(Clone)]
pub struct HostEnvBuilder {
    pwd: String,
    env: HashMap<String, String>,
}

impl Default for HostEnvBuilder {
    fn default() -> Self {
        Self {
            pwd: ".".to_string(),
            env: Default::default(),
        }
    }
}

impl HostEnvBuilder {
    pub fn build(self) -> HostEnv {
        HostEnv {
            pwd: self.pwd,
            env: self.env,
        }
    }
    pub fn pwd<S>(&mut self, pwd: S) -> &mut Self
    where
        S: ToString,
    {
        self.pwd = pwd.to_string();
        self
    }

    pub fn env<K, V>(&mut self, key: K, value: V) -> &mut Self
    where
        K: ToString,
        V: ToString,
    {
        self.env.insert(key.to_string(), value.to_string());
        self
    }
}

pub trait FileSystemFactory {
    fn create(
        &self,
        runtime: tokio::runtime::Handle,
    ) -> Result<Box<dyn virtual_fs::FileSystem + Send + Sync>, HostErr>;
}

struct RootFileSystemFactory {
    path: PathBuf,
}

impl RootFileSystemFactory {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl FileSystemFactory for RootFileSystemFactory {
    fn create(
        &self,
        handle: tokio::runtime::Handle,
    ) -> Result<Box<dyn FileSystem + Send + Sync>, HostErr> {
        match virtual_fs::host_fs::FileSystem::new(handle, self.path.clone()) {
            Ok(fs) => Result::Ok(Box::new(fs)),
            Err(err) => Result::Err(err.into()),
        }
    }
}

impl FileStoreCliExecutor {
    async fn assign<'a>(
        &self,
        ctx: &'a InCtx<'_, Assign>,
    ) -> Result<(), <FileStoreCliExecutor as Executor>::Err> {
        /*
        async fn wait(mut child: OsProcess, line: String) -> Result<(), StarErr> {
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

         */

        let bin = match &ctx.state {
            StateSrc::Substance(data) => data.to_bin()?,
            StateSrc::None => Box::new(Substance::Empty).to_bin()?,
        };
        let cli = Cli {
            command: Commands::Write { path: ctx.details.stub.point.to_path() },
        };
        let mut child = self.cli.execute(cli).await?;
        let mut stdin = child.stdin.take().ok_or(SpaceErr::from(format!(
            "command {} could not write to StdIn",
            cli.to_string()
        )))?;
        tokio::io::copy(&mut bin.as_bytes(), &mut stdin).await?;
        stdin.flush().await?;
        drop(stdin);
        Ok(())
    }
}

#[handler]
impl FileStoreCliExecutor {
    #[route("Hyp<Init>")]
    async fn handle_init(&self, ctx: InCtx<'_, HyperSubstance>) -> Result<(), StarErr> {
aprintln!("Hyp<Init>!!!!");

        let args = Cli::new(Commands::Init);
        let mut child = self.cli.execute(args).await?;
        child.close_stdin()?;
        child.wait().await?;
        Ok(())
    }
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

impl OsProcess {
    pub fn close_stdin(&mut self) -> Result<(), StarErr> {
        drop(self.child.stdin.take().unwrap());
        Ok(())
    }
}

impl Deref for OsProcess {
    type Target = Child;

    fn deref(&self) -> &Self::Target {
        &self.child
    }
}

impl DerefMut for OsProcess {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.child
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
        if !self.stub.loc.exists() {
            return Result::Err(StarErr::new(format!(
                "file not found: {}",
                self.stub.loc.display()
            )));
        }

        aprintln!("pwd: {}", env::current_dir().unwrap().display());
        aprintln!("self.stub.loc.exists(): {}", self.stub.loc.exists());
        aprintln!("self.stub.loc: {}", self.stub.loc.display());
        let mut command = Command::new(self.stub.loc.clone());

        command.envs(self.stub.env.env.clone());
        command.args(args);
        command.current_dir(self.stub.env.pwd.clone());
        command.env_clear();
        command.envs(&self.stub.env.env);
        aprintln!("GOT HERE...");
        //command.stdin(Stdio::piped()).output().await?;
        command.stdin(Stdio::piped()).output().await?;
        aprintln!("STDIN");
        //command.stdout(Stdio::piped()).output().await?;
        command.stdout(Stdio::piped()).output().await?;
        aprintln!("STDOUT");
        //command.stderr(Stdio::piped()).output().await?;
        command.stderr(Stdio::piped()).output().await?;
        aprintln!("STDERR");
        println!("{:?}", command);
        let child = command.spawn()?;
        aprintln!("child created...");
        Ok(OsProcess::new(child))
    }
}

#[derive(Clone)]
pub struct OsExeCli {
    pub stub: OsExeStub,
}

impl OsExeCli {
    pub fn new<I>(info: I) -> Self
    where
        I: Into<OsExeStub>,
    {
        let info = info.into();
        Self { stub: info }
    }
}

#[derive(DirectedHandler)]
pub struct FileStoreCliExecutor {
    pub cli: Box<
        dyn Executor<Args = Cli, Spawn = Result<OsProcess, StarErr>, Err = StarErr>
            + Send
            + Sync,
    >,
}

impl FileStoreCliExecutor {
    pub fn new(
        cli: Box<
            dyn Executor<Args = Cli, Spawn = Result<OsProcess, StarErr>, Err = StarErr>
                + Send
                + Sync,
        >,
    ) -> Self {
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

#[derive(Clone, Hash, Eq, PartialEq)]
pub enum HostApi {
    Cli(HostKind),
}

#[derive(Clone, Hash, Eq, PartialEq)]
pub enum HostKind {
    Os,
}

pub enum Host {
    Cli(CliHost),
}

impl Host {
    pub fn is_cli(&self) -> bool {
        match self {
            Host::Cli(_) => true,
        }
    }

    pub fn executor(
        &self,
    ) -> Option<
        Box<
            dyn Executor<Spawn = Result<OsProcess, StarErr>, Err = StarErr, Args = Vec<String>>
                + Send
                + Sync,
        >,
    > {
        match self {
            Host::Cli(CliHost::Os(exec)) => Some(Box::new(exec.clone())),
        }
    }
}

pub enum CliHost {
    Os(OsExeCli),
}

impl CliHost {
    pub fn executor(&self) -> &OsExeCli {
        match self {
            CliHost::Os(exec) => exec,
        }
    }
}

impl Host {}

impl HostApi {
    pub fn create<S>(&self, stub: S) -> Result<Host, StarErr>
    where
        S: Into<OsExeStub>,
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

impl Into<OsExeStub> for ExeStub<String, HostEnv, Option<Vec<String>>> {
    fn into(self) -> OsExeStub {
        OsExeStub::new(self.loc.into(), self.env.into(), ())
    }
}

#[derive(Clone, Hash, Eq, PartialEq)]
pub struct ExeStub<L, E, A>
where
    E: Clone + Hash + Eq + PartialEq,
    L: Clone + Hash + Eq + PartialEq,
    A: Clone + Hash + Eq + PartialEq,
{
    pub loc: L,
    pub env: E,
    pub args: A,
}

impl<L, E, A> ExeStub<L, E, A>
where
    E: Clone + Hash + Eq + PartialEq,
    L: Clone + Hash + Eq + PartialEq,
    A: Clone + Hash + Eq + PartialEq,
{
    pub fn new(loc: L, env: E, args: A) -> Self {
        Self { loc, env, args }
    }
}

pub fn stringify_args(args: Vec<&str>) -> Vec<String> {
    args.iter().map(|arg| arg.to_string()).collect()
}

impl<E> Into<ExeStub<PathBuf, OsEnv, ()>> for ExeStub<String, E, ()>
where
    E: Into<HostEnv> + Clone + Hash + Eq + PartialEq,
{
    fn into(self) -> ExeStub<PathBuf, HostEnv, ()> {
        ExeStub {
            loc: self.loc.into(),
            env: self.env.into(),
            args: (),
        }
    }
}

pub type OsExeInfo = ExeInfo<PathBuf, OsEnv, ()>;
pub type OsExeStub = ExeStub<PathBuf, OsEnv, ()>;
pub type OsExeStubArgs = ExeStub<PathBuf, HostEnv, Vec<String>>;

#[derive(Clone, Hash, Eq, PartialEq)]
pub struct ExeInfo<L, E, A>
where
    E: Clone + Hash + Eq + PartialEq,
    L: Clone + Hash + Eq + PartialEq,
    A: Clone + Hash + Eq + PartialEq,
{
    pub host: HostApi,
    pub stub: ExeStub<L, E, A>,
}

impl<L, E, A> ExeInfo<L, E, A>
where
    E: Clone + Hash + Eq + PartialEq,
    L: Clone + Hash + Eq + PartialEq,
    A: Clone + Hash + Eq + PartialEq,
{
    pub fn new(host: HostApi, stub: ExeStub<L, E, A>) -> Self {
        Self { host, stub }
    }
}

impl<L, E, A> ExeInfo<L, E, A>
where
    L: Clone + Hash + Eq + PartialEq + Into<PathBuf>,
    E: Clone + Hash + Eq + PartialEq + Into<HostEnv>,
    A: Clone + Hash + Eq + PartialEq,
{
    pub fn create_host(self) -> Result<Host, StarErr> {
        self.host.create(&self.stub)
    }
}

impl<L, E, A> From<&ExeStub<L, E, A>> for ExeStub<PathBuf, HostEnv, ()>
where
    L: Clone + Hash + Eq + PartialEq + Into<PathBuf>,
    E: Clone + Hash + Eq + PartialEq + Into<HostEnv>,
    A: Clone + Hash + Eq + PartialEq,
{
    fn from(stub: &ExeStub<L, E, A>) -> Self {
        let path = stub.loc.clone().into();
        let env = stub.env.clone().into();

        ExeStub::new(path, env, ())
    }
}