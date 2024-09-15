mod cache;
mod err;
pub mod src;

use crate::cache::{WasmModuleCache};
use crate::src::Source;
use err::Err;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::runtime::Handle;
use virtual_fs::{ClonableVirtualFile, FileSystem, Pipe, VirtualFile};
use wasmer::{Module, Store};
use wasmer_compiler_singlepass::Singlepass;
use wasmer_wasix::runtime::module_cache::ModuleCache;
use wasmer_wasix::runtime::task_manager::tokio::TokioTaskManager;
use wasmer_wasix::{PluggableRuntime, WasiEnv};

pub struct WasmService {
    store: Store,
    cache: Box<dyn WasmModuleCache>
}

impl  WasmService{
    pub fn new(cache: Box<dyn WasmModuleCache>) -> Self {
        let compiler = Singlepass::default();
        let store = Store::new(compiler);

        Self { store, cache }
    }

    pub async fn provision<S>(
        &self,
        wasm: S,
        host_config: WasmHostConfig
    ) -> Result<WasmHost, Err> where S: ToString{

        let store = Store::default();
        //let module = self.cache.get(wasm.to_string().as_str()).await?;

        let wasm_bytes = fs::read("filestore.wasm").await?;
        let module = Module::new(& store,wasm_bytes).unwrap();

        Result::Ok(WasmHost::new(module, host_config,store))
    }
}

pub struct Process {
    stdin: Pipe,
    stdout: Pipe,
    stderr: Pipe
}

impl Process {
    pub fn stdin(&mut self) -> &mut Pipe {
        &mut self.stdin
    }

    pub fn stdout(&mut self) -> &mut Pipe {
        &mut self.stdout
    }

    pub fn stderr(&mut self) -> &mut Pipe {
        &mut self.stderr
    }

    pub async fn direct_stdin(&mut self, data: String) -> Result<(), Err> {
        writeln!(self.stdin, "{}", data)?;
        Result::Ok(())
    }
}

pub trait FileSystemFactory {
    fn create(
        &self,
        runtime: tokio::runtime::Handle
    ) -> Result<Box<dyn virtual_fs::FileSystem + Send + Sync>,Err>;
}

struct RootFileSystemFactory {
    path: PathBuf
}

impl RootFileSystemFactory {
    pub fn new( path: PathBuf ) -> Self {
        Self {
            path
        }
    }
}

impl FileSystemFactory for RootFileSystemFactory {
    fn create(&self, handle: tokio::runtime::Handle) -> Result<Box<dyn FileSystem + Send + Sync>,Err> {
        match virtual_fs::host_fs::FileSystem::new(handle, self.path.clone()) {
            Ok(fs) => {
                Result::Ok(Box::new(fs))
            }
            Err(err) => {
                Result::Err(err.into())
            }
        }
    }
}

pub struct WasmHost {
    store: Store,
    module: Module,
    config: WasmHostConfig,
    runtime: Arc<PluggableRuntime>,
}

impl WasmHost {
    fn new(module: Module, config: WasmHostConfig, store: Store) -> Self {
        let runtime = Arc::new(PluggableRuntime::new(Arc::new(TokioTaskManager::new(
            Handle::current(),
        ))));
        Self {
            store,
            module,
            config,
            runtime,
        }
    }

    pub async fn execute<L>(& mut self, line: L) -> Result<Process, Err>
    where
        L: ToString,
    {
        let mut builder = WasiEnv::builder("wasm program").args(&[line.to_string().as_str()]);

        builder = builder.env("PWD", "/");

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
            builder = builder.fs( Box::new(fs_config.fs_factory.create(Handle::current().clone()).unwrap()));
        };

        if self.config.runtime {
           builder = builder.runtime(self.runtime.clone());
        }

        builder = builder.current_dir("/");
        //builder = builder.stdout(Box::new(io::stdout()));

        //builder.run(self.module.clone())?;
        builder.run_with_store(self.module.clone(), & mut self.store )?;

        Ok(Process {
            stdin: stdin_tx,
            stdout: stdout_rx,
            stderr: stderr_rx,
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
    pub fs: Option<FsConfig>,
    pub runtime: bool,
}

impl Default for WasmHostConfig {
    fn default() -> Self {
        Self {
            runtime: false,
            fs: Option::None,
        }
    }
}

impl WasmHostConfig {
    pub fn builder() -> WasmHostConfigBuilder {
        WasmHostConfigBuilder::new()
    }
}

pub struct WasmHostConfigBuilder {
    pub runtime: bool,
    pub fs: Option<FsConfigBuilder>,
}

impl WasmHostConfigBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn fs<F>(&mut self, factory: Arc<dyn FileSystemFactory>, f: F) -> &mut Self
    where
        F: FnOnce(&mut FsConfigBuilder)
    {
        if self.fs.is_none() {
            self.fs = Option::Some(FsConfigBuilder::new(factory));
        }

        f(self.fs.as_mut().unwrap());
        self
    }
    pub fn runtime(&mut self, runtime: bool) -> &mut Self {
        self.runtime = runtime;
        self
    }

    pub fn build(self) -> WasmHostConfig {
        WasmHostConfig {
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

    pub fn preopen<S>(&mut self, dir: S) -> &mut Self where S: ToString{
        self.pre_opened_dirs.push(dir.to_string());
        self
    }

    pub fn build(self) -> FsConfig {
        FsConfig {
            fs_factory: self.fs_factory,
            pre_opened_dirs: self.pre_opened_dirs,
        }
    }
}


#[cfg(test)]
pub mod test {
    use std::io::Read;
    use crate::cache::WasmModuleMemCache;
    use crate::src::FileSystemSrc;
    use crate::{RootFileSystemFactory, WasmHostConfig, WasmService};
    use std::sync::Arc;
    use tokio::io::AsyncReadExt;

    #[tokio::test]
    pub async fn test() {

        println!("starting test");
        let source = Box::new(FileSystemSrc::new("."));
        let cache = Box::new(WasmModuleMemCache::new_with_ser(source, ".".into()));
        let service = WasmService::new(cache);
        let mut builder = WasmHostConfig::builder();
        let fs_factory = Arc::new(RootFileSystemFactory::new("./".into()));
        builder.fs( fs_factory, |fs_builder|{
           fs_builder.preopen("./");
        });
        builder.runtime(false);
        let config = builder.build();
        let mut host = service.provision( "filestore.wasm", config ).await.unwrap();

        let mut process = host.execute("pwd").await.unwrap();

        process.stdin.close();
        println!("it worked i guess?");

        let mut both = AsyncReadExt::chain(process.stdout,process.stdin);

        let mut out = String::new();
        both.read_to_string(&mut out).await.unwrap();

        println!("{}",out);


    }
}