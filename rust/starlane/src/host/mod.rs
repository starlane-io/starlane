use crate::host::err::HostErr;
use crate::hyperspace::err::HyperErr;
use crate::hyperspace::service::ExeInfo;
use crate::hyperspace::service::Executor;
use itertools::Itertools;
use starlane_space::wave::exchange::asynch::DirectedHandler;
use std::collections::HashMap;
use std::env;
use std::fmt::Write;
use std::hash::{Hash, Hasher};
use std::io::Read;
use std::path::PathBuf;
use virtual_fs::FileSystem;

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

#[async_trait]
pub trait ExeService
where
    Self::ExeInfo: Clone + Hash + Eq + PartialEq,
    Self::Executor: Executor,
    Self::Err: HyperErr,
{
    type ExeInfo;
    type Executor;
    type Err;
    async fn provision(&mut self, exe: Self::ExeInfo) -> Result<Self::Executor, HostErr>;
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
