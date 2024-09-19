use crate::host::err::HostErr;
use itertools::Itertools;
use std::collections::HashMap;
use std::env;
use std::fmt::Write;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{Stderr, Stdin, Stdout};
use tokio::process::{Child, ChildStderr, ChildStdout, ChildStdin, Command};
use virtual_fs::{FileSystem, Pipe};
use crate::hyper::space::err::HyperErr;

pub mod err;
pub mod ext;

pub mod wasm;

#[derive(Clone, Eq, PartialEq, Hash)]
pub struct ExtKey<B>
where
    B: Hash + Eq + PartialEq,
{
    bin: B,
    env: ExeEnv,
}

impl<B> ExtKey<B>
where
    B: Clone + Hash + Eq + PartialEq,
{
    pub fn new(bin: B, env: ExeEnv) -> Self {
        Self { bin, env }
    }
}

#[async_trait]
pub trait ExeService where Self::Bin: Into<PathBuf>, Self::Executor: Executor, Self::Err: HyperErr
{
    type Bin;
    type Executor;
    type Err;
    async fn provision(&mut self, bin: Self::Bin, env: ExeEnv) -> Result<Self::Executor, HostErr>;
}


#[async_trait]
pub trait Executor
where Self::Err: HyperErr,  Self::Proc: Proc{
    type Err;
    type Proc;
    async fn execute(&self, args: Vec<String>) -> Result<Self::Proc, Self::Err>;
}

pub struct ExtExecutor {
    pub path: PathBuf,
    pub env: ExeEnv,
}

impl Executor for ExtExecutor {
    type Err = HostErr;
    type Proc = ExtProcess;

    async fn execute(&self, args: Vec<String>) -> Result<Self::Proc, Self::Err> {
        let mut command = Command::new(self.path.clone());
        command.envs(self.env.env.clone());
        command.args(args);
        command.current_dir(self.env.pwd.clone());
        command.env_clear();
        command.stdin(Stdio::piped()).output().await?;
        command.stdout(Stdio::piped()).output().await?;
        command.stderr(Stdio::piped()).output().await?;

        let child = command.spawn()?;
        Ok(Self::Proc::new(child))
    }
}

pub trait Proc where {
    type StdOut;
    type StdErr;
    type StdIn;
    fn stderr(&self) -> Option<&Self::StdErr>;
    fn stdout(&self) -> Option<&Self::StdOut>;

    fn stdin(&mut self) -> Option<&Self::StdIn>;
}



#[derive(Clone, Eq, PartialEq)]
pub struct ExeEnv {
    pwd: String,
    env: HashMap<String, String>,
}

impl Hash for ExeEnv {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_str(self.pwd.as_str());
        for key in self.env.keys().sorted() {
            state.write_str(key.as_str());
            state.write_str(self.env.get(key).unwrap());
        }
    }
}

impl Default for ExeEnv {
    fn default() -> Self {
        Self {
            pwd: env::current_dir().unwrap().to_str().unwrap().to_string(),
            env: HashMap::default(),
        }
    }
}

#[derive(Clone)]
pub struct ExtEnvBuilder {
    pwd: String,
    env: HashMap<String, String>,
}

impl ExtEnvBuilder {
    pub fn build(self) -> ExeEnv {
        ExeEnv {
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

impl Default for ExtEnvBuilder {
    fn default() -> Self {
        Self {
            pwd: env::current_dir().unwrap().to_str().unwrap().to_string(),
            env: Default::default(),
        }
    }
}

pub struct ExtProcess {
    child: Child,
}

impl ExtProcess {
    pub fn new( child: Child) -> Self {
        Self {
            child
        }
    }
}


impl Proc for ExtProcess {
    type StdOut= ChildStdout;
    type StdIn= ChildStdin;
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

