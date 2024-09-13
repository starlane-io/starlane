mod cache;
mod err;
pub mod src;

use crate::cache::WasmModuleCache;
use err::Err;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::runtime::{Handle, Runtime};
use virtual_fs::{ClonableVirtualFile, Pipe, VirtualFile};
use wasmer::Module;
use wasmer_wasix::runtime::task_manager::tokio::TokioTaskManager;
use wasmer_wasix::{PluggableRuntime, WasiEnv};

pub struct WasmService {
    cache: Box<dyn WasmModuleCache>,
    runtime: Arc<Runtime>,
}

impl WasmService {
    pub fn new(cache: Box<dyn WasmModuleCache>) -> Self {
        let runtime = Arc::new(Runtime::new().unwrap());
        Self { cache, runtime }
    }

    pub async fn provision(
        &self,
        wasm: String,
        host_config: WasmHostConfig,
    ) -> Result<WasmHost, Err> {
        let module = self.cache.get(wasm.as_str()).await?;
        Result::Ok(WasmHost::new(module, host_config))
    }
}

pub struct Run {}

pub struct Process {
    stdin: Box<dyn VirtualFile + Send + Sync + 'static>,
    stdout: Box<dyn VirtualFile + Send + Sync + 'static>,
    stderr: Box<dyn VirtualFile + Send + Sync + 'static>,
}

impl Process {
    pub fn stdin(&mut self) -> &mut Box<dyn VirtualFile + Send + Sync + 'static> {
        &mut self.stdin
    }

    pub fn stdout(&mut self) -> &mut Box<dyn VirtualFile + Send + Sync + 'static> {
        &mut self.stdout
    }

    pub fn stderr(&mut self) -> &mut Box<dyn VirtualFile + Send + Sync + 'static> {
        &mut self.stderr
    }

    pub async fn direct_stdin(&mut self, data: Vec<u8>) -> Result<(), Err> {
        self.stdin.write_all(data.as_slice()).await?;
        Result::Ok(())
    }
}

pub trait FileSystemFactory {
    fn create(
        &self,
        runtime: Arc<dyn wasmer_wasix::Runtime>,
    ) -> Box<dyn virtual_fs::FileSystem + Send + Sync>;
}

pub struct WasmHost {
    module: Module,
    config: WasmHostConfig,
    runtime: Arc<PluggableRuntime>,
}

impl WasmHost {
    pub fn new(module: Module, config: WasmHostConfig) -> Self {
        let runtime = Arc::new(PluggableRuntime::new(Arc::new(TokioTaskManager::new(
            Handle::current(),
        ))));
        Self {
            module,
            config,
            runtime,
        }
    }

    pub async fn execute<L>(&self, line: L) -> Result<Process, Err>
    where
        L: ToString,
    {
        let mut builder = WasiEnv::builder("wasm program").args(&[line.to_string().as_str()]);

        let (stdin_tx, stdin_rx) = Pipe::channel();
        let (stdout_tx, stdout_rx) = Pipe::channel();
        let (stderr_tx, stderr_rx) = Pipe::channel();

        builder = builder
            .stdin(Box::new(stdin_rx))
            .stdout(Box::new(stdout_tx))
            .stderr(Box::new(stderr_tx));

        if let Option::Some(ref fs_config) = self.config.fs {
            for d in &fs_config.pre_opened_dirs {
                builder = builder.preopen_dir(Path::new(d))?;
            }
            builder = builder.fs(fs_config.fs_factory.create(self.runtime.clone()));
        };

        if self.config.runtime {
            //            let mut rt = Arc::new(PluggableRuntime::new(Arc::new(TokioTaskManager::new(Handle::current()))));
            builder = builder.runtime(self.runtime.clone());
        }

        builder.run(self.module.clone())?;

        Ok(Process {
            stdin: Box::new(stdin_tx),
            stdout: Box::new(stdout_rx),
            stderr: Box::new(stderr_rx),
        })
    }
}

pub struct WasmHostBuilder {}

impl WasmHostBuilder {}

#[derive(Clone)]
enum WasmInterfaceKind {
    Cli,
}

impl WasmInterfaceKind {}

#[derive(Clone)]
pub struct WasmHostConfig {
    pub interface: WasmInterfaceKind,
    pub fs: Option<FsConfig>,
    pub runtime: bool,
}

impl Default for WasmHostConfig {
    fn default() -> Self {
        Self {
            runtime: false,
            interface: WasmInterfaceKind::Cli,
            fs: Option::None,
        }
    }
}

impl WasmHostConfig {}

pub struct WasmHostConfigBuilder {
    pub runtime: bool,
    pub interface: WasmInterfaceKind,
    pub fs: Option<FsConfigBuilder>,
}

impl WasmHostConfigBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn fs<F>(&mut self, factory: Arc<dyn FileSystemFactory>, f: F) -> &mut Self
    where
        F: FnOnce(&mut FsConfigBuilder) -> &mut Self,
    {
        if self.fs.is_none() {
            self.fs = Option::Some(FsConfigBuilder::new(factory));
        }

        f(self.fs.as_mut().unwrap())
    }
    pub fn runtime(&mut self, runtime: bool) -> &mut Self {
        self.runtime = runtime;
        self
    }

    pub fn build(self) -> WasmHostConfig {
        WasmHostConfig {
            interface: self.interface,
            fs: match self.fs {
                None => None,
                Some(builder) => Some(builder.build()),
            },
            runtime: false,
        }
    }
}

impl Default for WasmHostConfigBuilder {
    fn default() -> Self {
        WasmHostConfigBuilder {
            runtime: false,
            interface: WasmInterfaceKind::Cli,
            fs: None,
        }
    }
}

#[derive(Clone)]
pub struct FsConfig {
    pub fs_factory: Arc<dyn FileSystemFactory>,
    pub pre_opened_dirs: Vec<String>,
}

pub enum FsKind {
    Local,
}

impl Default for FsKind {
    fn default() -> Self {
        FsKind::Local
    }
}

pub struct FsConfigBuilder {
    fs_factory: Arc<dyn FileSystemFactory>,
    kind: FsKind,
    pre_opened_dirs: Vec<String>,
}

impl FsConfigBuilder {
    pub fn new(fs_factory: Arc<dyn FileSystemFactory>) -> Self {
        Self {
            fs_factory,
            kind: Default::default(),
            pre_opened_dirs: vec![],
        }
    }

    pub fn preopen(&mut self, dir: String) -> &mut Self {
        self.pre_opened_dirs.push(dir);
        self
    }

    pub fn build(self) -> FsConfig {
        FsConfig {
            fs_factory: self.fs_factory,
            pre_opened_dirs: self.pre_opened_dirs,
        }
    }
}
