use std::collections::HashMap;
use std::env;
use std::fmt::Write;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use itertools::Itertools;
use virtual_fs::{FileSystem, Pipe};
use crate::host::err::HostErr;

pub mod err;
pub mod ext;

pub mod wasm;

#[derive(Clone,Eq, PartialEq,Hash)]
pub struct HostKey<B>
where
    B: Hash + Eq + PartialEq,
{
    bin: B,
    env: HostEnv,
}

impl<B> HostKey<B>
where
    B: Clone + Hash + Eq + PartialEq,
{
    pub fn new(bin: B, env: HostEnv) -> Self {
        Self { bin, env }
    }
}

#[async_trait]
pub trait HostService<B, P, S> {
    async fn provision(&mut self, bin: B, env: HostEnv) -> Result<Box<dyn Host<P,S>>, HostErr>;
}

#[async_trait]
pub trait Host<P, S> {
    async fn execute(&self, args: Vec<String>) -> Result<P, HostErr>;

    fn direct(&self) -> Box<dyn StdinProc<S>>;
}

#[async_trait]
pub trait StdinProc<P> {
    fn stdin(&self) -> P;

    async fn execute(self, args: Vec<String>) -> Result<P, HostErr>;
}

#[derive(Clone, Eq, PartialEq)]
pub struct HostEnv {
    pwd: String,
    env: HashMap<String, String>,
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

impl Default for HostEnvBuilder {
    fn default() -> Self {
        Self {
            pwd: env::current_dir().unwrap().to_str().unwrap().to_string(),
            env: Default::default(),
        }
    }
}

pub struct Process {
    stdout: Pipe,
    stderr: Pipe,
}

impl Process {
    pub fn stdout(&mut self) -> &mut Pipe {
        &mut self.stdout
    }

    pub fn stderr(&mut self) -> &mut Pipe {
        &mut self.stderr
    }

    /*
    pub async fn direct_stdin(&mut self, data: String) -> Result<(), HostErr> {
        writeln!(self.stdin, "{}", data)?;
        Result::Ok(())
    }

     */
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

