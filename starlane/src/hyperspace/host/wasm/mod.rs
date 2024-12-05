mod cache;
mod source;

use crate::hyperspace::host::err::HostErr;
use crate::hyperspace::host::wasm::cache::WasmModuleCache;
use crate::hyperspace::host::FileSystemFactory;
use crate::hyperspace::hyperspace::service::OsProcess;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use tokio::runtime::Handle;
use virtual_fs::Pipe;
use wasmer::{Module, Store};
use wasmer_compiler_singlepass::Singlepass;
use wasmer_wasix::runtime::task_manager::tokio::TokioTaskManager;
use wasmer_wasix::{PluggableRuntime, WasiEnv};

pub struct WasmService {
    store: Store,
    cache: Box<dyn WasmModuleCache>,
}

impl WasmService {
    pub fn new(cache: Box<dyn WasmModuleCache>) -> Self {
        let compiler = Singlepass::default();
        let store = Store::new(compiler);

        Self { store, cache }
    }

    pub async fn provision<S>(
        &mut self,
        wasm: S,
        host_config: WasmHostConfig,
    ) -> Result<WasmHost, HostErr>
    where
        S: ToString,
    {
        let store = Store::new(Singlepass::default());
        //let module = self.cache.get(wasm.to_string().as_str()).await?;

        let module = self.cache.get("filestore.wasm", &store).await?;
        //let wasm_bytes = fs::read("filestore.wasm").await?;
        //let module = Module::new(& store,wasm_bytes).unwrap();

        Result::Ok(WasmHost::new(module, host_config, store))
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
    pub async fn execute<I, Arg>(&mut self, args: I) -> Result<OsProcess, HostErr>
    where
        I: IntoIterator<Item=Arg>,
        Arg: AsRef<[u8]>,
    {
        self.execute_with_data(args, &[]).await
    }

    pub async fn execute_with_data<I, Arg>(
        &mut self,
        args: I,
        stdin: &[u8],
    ) -> Result<OsProcess, HostErr>
    where
        I: IntoIterator<Item=Arg>,
        Arg: AsRef<[u8]>,
    {
        let mut builder = WasiEnv::builder("wasm program").args(args);

        let (mut stdin_tx, stdin_rx) = Pipe::channel();
        let (stdout_tx, stdout_rx) = Pipe::channel();
        let (stderr_tx, stderr_rx) = Pipe::channel();

        std::io::Write::write_all(&mut stdin_tx, stdin).unwrap();
        //        stdin_tx.flush().unwrap();
        stdin_tx.close();

        builder = builder
            .stdin(Box::new(stdin_rx))
            .stdout(Box::new(stdout_tx))
            .stderr(Box::new(stderr_tx));

        if let Option::Some(ref fs_config) = self.config.fs {
            for d in &fs_config.pre_opened_dirs {
                builder = builder.preopen_dir(Path::new(d))?;
            }
            builder = builder.fs(Box::new(
                fs_config
                    .fs_factory
                    .create(Handle::current().clone())
                    .unwrap(),
            ));

            builder = builder.env("PWD", fs_config.pwd.clone());
        };

        if self.config.runtime {
            builder = builder.runtime(self.runtime.clone());
        }

        builder = builder.current_dir("/");
        //builder = builder.stdout(Box::new(io::stdout()));

        //builder.run(self.module.clone())?;
        builder.run_with_store(self.module.clone(), &mut self.store)?;

        /*
        Ok(OsProcess {
            stdout: stdout_rx,
            stderr: stderr_rx,
        })

         */
        todo!()
    }

    pub async fn execute_with_stdin<I, Arg>(
        &mut self,
        args: I,
        stdin: Pipe,
    ) -> Result<OsProcess, HostErr>
    where
        I: IntoIterator<Item=Arg>,
        Arg: AsRef<[u8]>,
    {
        let mut builder = WasiEnv::builder("wasm program").args(args);

        let (stdout_tx, stdout_rx) = Pipe::channel();
        let (stderr_tx, stderr_rx) = Pipe::channel();

        builder = builder
            .stdin(Box::new(stdin))
            .stdout(Box::new(stdout_tx))
            .stderr(Box::new(stderr_tx));

        if let Option::Some(ref fs_config) = self.config.fs {
            for d in &fs_config.pre_opened_dirs {
                builder = builder.preopen_dir(Path::new(d))?;
            }
            builder = builder.fs(Box::new(
                fs_config
                    .fs_factory
                    .create(Handle::current().clone())
                    .unwrap(),
            ));

            builder = builder.env("PWD", fs_config.pwd.clone());
        };

        if self.config.runtime {
            builder = builder.runtime(self.runtime.clone());
        }

        builder = builder.current_dir("/");
        //builder = builder.stdout(Box::new(io::stdout()));

        //builder.run(self.module.clone())?;
        builder.run_with_store(self.module.clone(), &mut self.store)?;

        /*
        Ok(OsProcess {
            stdout: stdout_rx,
            stderr: stderr_rx,
        })
         */
        todo!()
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
    pub pwd: String,
    pub fs: Option<FsConfigBuilder>,
}

impl WasmHostConfigBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn fs<F>(&mut self, factory: Arc<dyn FileSystemFactory>, f: F) -> &mut Self
    where
        F: FnOnce(&mut FsConfigBuilder),
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
            pwd: ".".to_string(),
            runtime: false,
            fs: None,
        }
    }
}

#[derive(Clone)]
pub struct FsConfig {
    pub fs_factory: Arc<dyn FileSystemFactory>,
    pub pre_opened_dirs: Vec<String>,
    pub pwd: String,
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
    pwd: String,
}

impl FsConfigBuilder {
    pub fn new(fs_factory: Arc<dyn FileSystemFactory>) -> Self {
        Self {
            fs_factory,
            kind: Default::default(),
            pre_opened_dirs: vec![],
            pwd: "./".into(),
        }
    }

    pub fn preopen<S>(&mut self, dir: S) -> &mut Self
    where
        S: ToString,
    {
        self.pre_opened_dirs.push(dir.to_string());
        self
    }

    pub fn pwd<S>(&mut self, dir: S) -> &mut Self
    where
        S: ToString,
    {
        self.pwd = dir.to_string();
        self
    }

    pub fn build(self) -> FsConfig {
        FsConfig {
            pwd: self.pwd,
            fs_factory: self.fs_factory,
            pre_opened_dirs: self.pre_opened_dirs,
        }
    }
}

#[cfg(test)]
pub mod test {
    use crate::hyperspace::host::wasm::cache::WasmModuleMemCache;
    use crate::hyperspace::host::wasm::source::FileSystemSrc;
    use crate::hyperspace::host::wasm::{WasmHostConfig, WasmService};
    use crate::hyperspace::host::{FileSystemFactory, RootFileSystemFactory};
    use tokio::io::AsyncReadExt;
    use tokio::runtime::Handle;
    use virtual_fs::{FileSystem, Pipe};

    #[tokio::test]
    pub async fn test_fs() {
        // tokio::fs::remove_dir_all("./test-dir").await;
        //tokio::fs::create_dir_all("./test-dir").await;
        let fs_factory = Arc::new(RootFileSystemFactory::new("./test-dir".into()));
        let fs = fs_factory.create(Handle::current()).unwrap();
        let paths = fs.read_dir(Path::new("/test-dir")).unwrap();
        for path in paths {
            let path = path.unwrap().path();
            println!("{}", path.display());
        }
        let mut file = fs.new_open_options().open("blah.txt").unwrap();

        //virtual_fs::AsyncWriteExt::write_all(& mut file, "hello".as_bytes()).await.unwrap();
    }

    #[tokio::test]
    pub async fn test() {
        tokio::fs::remove_dir_all("./test-dir").await;
        tokio::fs::create_dir_all("./test-dir").await;

        /*
        tokio::fs::create_dir_all("./test-dir/subdir").await;
        tokio::fs::write("./test-dir/test1.txt", "test1").await;
        tokio::fs::write("./test-dir/test2.txt", "test2").await;
        tokio::fs::write("./test-dir/subdir/sub1.txt", "test1").await;
        tokio::fs::write("./test-dir/subdir/sub2.txt", "test2").await;

         */
        println!("starting test");
        let source = Box::new(FileSystemSrc::new("."));
        let cache = Box::new(WasmModuleMemCache::new_with_ser(source, ".".into()));
        let mut service = WasmService::new(cache);
        let mut builder = WasmHostConfig::builder();
        let fs_factory = Arc::new(RootFileSystemFactory::new("./test-dir".into()));
        builder.fs(fs_factory, |fs_builder| {
            fs_builder.preopen("/");
            fs_builder.pwd("/");
        });
        builder.runtime(false);
        let config = builder.build();
        let mut host = service.provision("filestore.wasm", config).await.unwrap();

        //        let mut process = host.execute(&["test"]).await.unwrap();
        let mut process = host.execute(&["mkdir", "subdir"]).await.unwrap();
        let mut process = host.execute(&["exists", "subdir"]).await.unwrap();
        assert!(host.execute(&["exists", "badfile"]).await.is_err());

        let (mut stdin_tx, stdin_rx) = Pipe::channel();

        std::io::Write::write_all(&mut stdin_tx, "hello you happy people".as_bytes()).unwrap();
        stdin_tx.close();
        host.execute_with_stdin(&["write", "/file1.txt"], stdin_rx)
            .await
            .unwrap();

        //        let mut process = host.execute(&["list"]).await.unwrap();

        // process.stdin.close();
        println!("it worked i guess?");

        let mut both = AsyncReadExt::chain(process.stdout, process.stderr);

        let mut out = String::new();
        both.read_to_string(&mut out).await.unwrap();

        println!("{}", out);

        let mut err = String::new();
        both.read_to_string(&mut err).await.unwrap();

        println!("{}", err);
    }
}
