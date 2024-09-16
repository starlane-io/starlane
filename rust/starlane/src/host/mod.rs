use std::collections::HashMap;
use std::env;
use std::fmt::Write;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use wasmer::Store;
use wasmer_compiler_singlepass::Singlepass;
use crate::host::cache::WasmModuleCache;

pub mod cache;
pub mod err;
pub mod ext;
pub mod src;


#[derive(Clone,Eq, PartialEq,Hash)]
pub struct HostKey<B>
where
    B: Hash + Eq + PartialEq,
{
    bin: B,
    env: Env,
}

impl<B> HostKey<B>
where
    B: Clone + Hash + Eq + PartialEq,
{
    pub fn new(bin: B, env: Env) -> Self {
        Self { bin, env }
    }
}

#[async_trait]
pub trait HostService<B, P> {
    async fn provision(&mut self, bin: B, env: Env) -> Result<Box<dyn Host<P>>, Err>;
}

#[async_trait]
pub trait Host<P, S> {
    async fn execute(&self, args: Vec<String>) -> Result<P, Err>;

    fn direct(&self) -> Box<dyn StdinProc<S>>;
}

#[async_trait]
pub trait StdinProc<P> {
    fn stdin(&self) -> P;

    async fn execute(self, args: Vec<String>) -> Result<P, Err>;
}

#[derive(Clone, Eq, PartialEq)]
pub struct Env {
    pwd: String,
    env: HashMap<String, String>,
}


impl Default for Env {
    fn default() -> Self {
        Self {
            pwd: env::current_dir().unwrap().to_str().unwrap().to_string(),
            env: HashMap::default(),
        }
    }
}

#[derive(Clone)]
pub struct EnvBuilder {
    pwd: String,
    env: HashMap<String, String>,
}

impl EnvBuilder {
    pub fn build(self) -> Env {
        Env {
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

impl Default for EnvBuilder {
    fn default() -> Self {
        Self {
            pwd: env::current_dir().unwrap().to_str().unwrap().to_string(),
            env: Default::default(),
        }
    }
}

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
    ) -> Result<WasmHost, Err>
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
    pub async fn direct_stdin(&mut self, data: String) -> Result<(), Err> {
        writeln!(self.stdin, "{}", data)?;
        Result::Ok(())
    }

     */
}

pub trait FileSystemFactory {
    fn create(
        &self,
        runtime: tokio::runtime::Handle,
    ) -> Result<Box<dyn virtual_fs::FileSystem + Send + Sync>, Err>;
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
    ) -> Result<Box<dyn FileSystem + Send + Sync>, Err> {
        match virtual_fs::host_fs::FileSystem::new(handle, self.path.clone()) {
            Ok(fs) => Result::Ok(Box::new(fs)),
            Err(err) => Result::Err(err.into()),
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
    pub async fn execute<I, Arg>(&mut self, args: I) -> Result<Process, Err>
    where
        I: IntoIterator<Item = Arg>,
        Arg: AsRef<[u8]>,
    {
        self.execute_with_data(args, &[]).await
    }

    pub async fn execute_with_data<I, Arg>(&mut self, args: I, stdin: &[u8]) -> Result<Process, Err>
    where
        I: IntoIterator<Item = Arg>,
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

        Ok(Process {
            stdout: stdout_rx,
            stderr: stderr_rx,
        })
    }

    pub async fn execute_with_stdin<I, Arg>(&mut self, args: I, stdin: Pipe) -> Result<Process, Err>
    where
        I: IntoIterator<Item = Arg>,
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

        Ok(Process {
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
    use crate::cache::WasmModuleMemCache;
    use crate::src::FileSystemSrc;
    use crate::{FileSystemFactory, RootFileSystemFactory, WasmHostConfig, WasmService};
    use std::io::Write;
    use std::path::Path;
    use std::sync::Arc;
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
