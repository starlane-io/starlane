use std::collections::HashMap;
use std::convert::TryInto;
use std::hash::Hasher;
use std::ops::Deref;
use std::str::FromStr;
use std::sync::Arc;
use std::thread;

use futures::FutureExt;
use mesh_portal_serde::version::latest::bin::Bin;
use mesh_portal_serde::version::latest::id::Address;
use mesh_portal_serde::version::latest::path::Path;
use mesh_portal_serde::version::latest::payload::Primitive;
use tokio::io::AsyncReadExt;
use tokio::runtime::Handle;
use tokio::sync::{broadcast, mpsc, oneshot};
use wasmer::{Cranelift, Store, Universal};


use crate::artifact::ArtifactRef;
use crate::config::app::{AppConfig, AppConfigParser};
use crate::config::bind::{BindConfig, BindConfigParser };
use crate::config::mechtron::{MechtronConfig, MechtronConfigParser};
use crate::config::wasm::{Wasm, WasmCompiler};
use crate::error::Error;
use crate::file_access::FileAccess;
use crate::resource::{Kind, ResourceRecord};
use crate::resource::ArtifactKind;
use crate::resource::config::Parser;
use crate::starlane::api::StarlaneApi;
use crate::starlane::StarlaneMachine;
use crate::util::{AsyncHashMap, AsyncProcessor, AsyncRunner, Call};

pub type Data = Arc<Vec<u8>>;
pub type ZipFile = Address;

pub trait Cacheable: Send + Sync + 'static {
    fn artifact(&self) -> ArtifactRef;
    fn references(&self) -> Vec<ArtifactRef>;
}

pub struct ProtoArtifactCachesFactory {
    root_caches: Arc<RootArtifactCaches>,
}

impl ProtoArtifactCachesFactory {
    pub fn new(
        src: ArtifactBundleSrc,
        file_access: FileAccess,
        machine: StarlaneMachine,
    ) -> Result<ProtoArtifactCachesFactory, Error> {
        let bundle_cache = ArtifactBundleCache::new(src, file_access, machine, AuditLogger::new())?;
        Ok(Self {
            root_caches: Arc::new(RootArtifactCaches::new(bundle_cache)),
        })
    }

    pub fn create(&self) -> ProtoArtifactCaches {
        ProtoArtifactCaches::new(self.root_caches.clone())
    }
}

pub struct ArtifactCaches {
    pub raw: ArtifactItemCache<Raw>,
    pub app_configs: ArtifactItemCache<AppConfig>,
    pub mechtron_configs: ArtifactItemCache<MechtronConfig>,
    pub bind_configs: ArtifactItemCache<BindConfig>,
    pub wasms: ArtifactItemCache<Wasm>,
//    pub http_router_config: ArtifactItemCache<HttpRouterConfig>,
}

impl ArtifactCaches {
    fn new() -> Self {
        ArtifactCaches {
            raw: ArtifactItemCache::new(),
            app_configs: ArtifactItemCache::new(),
            mechtron_configs: ArtifactItemCache::new(),
            bind_configs: ArtifactItemCache::new(),
            wasms: ArtifactItemCache::new(),
 //           http_router_config: ArtifactItemCache::new()
        }
    }
}

pub struct ArtifactItemCache<C: Cacheable> {
    map: HashMap<Address, ArtifactItem<C>>,
}

impl<C: Cacheable> ArtifactItemCache<C> {
    fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn get(&self, artifact: &Address) -> Option<ArtifactItem<C>> {
        self.map.get(&artifact).cloned()
    }

    fn add(&mut self, item: ArtifactItem<C>) {
        self.map.insert(item.artifact().address, item);
    }
}

pub struct ProtoArtifactCaches {
    root_caches: Arc<RootArtifactCaches>,
    proc_tx: mpsc::Sender<ProtoArtifactCall>,
    claims: AsyncHashMap<ArtifactRef, Claim>,
}

impl ProtoArtifactCaches {
    fn new(root_caches: Arc<RootArtifactCaches>) -> Self {
        let claims = AsyncHashMap::new();
        let (proc_tx, proc_rx) = mpsc::channel(1024);
        AsyncRunner::new(
            Box::new(ProtoArtifactCacheProc::new(
                root_caches.clone(),
                claims.clone(),
                proc_tx.clone(),
            )),
            proc_tx.clone(),
            proc_rx,
        );

        ProtoArtifactCaches {
            root_caches: root_caches,
            proc_tx: proc_tx,
            claims,
        }
    }

    pub async fn cache(&mut self, artifacts: Vec<ArtifactRef>) -> Result<(), Error> {
        let (tx, rx) = oneshot::channel();
        self.proc_tx
            .send(ProtoArtifactCall::Cache { artifacts, tx })
            .await;
        rx.await?
    }

    pub async fn to_caches(self) -> Result<ArtifactCaches, Error> {
        let mut caches = ArtifactCaches::new();
        let claims = self.claims.into_map().await?;

        for (artifact, _claim) in claims {
            match artifact.kind {
                ArtifactKind::AppConfig => {
                    caches
                        .app_configs
                        .add(self.root_caches.app_configs.get(artifact).await?);
                }
                ArtifactKind::Raw => {
                    caches.raw.add(self.root_caches.raw.get(artifact).await?);
                }
                ArtifactKind::MechtronConfig => {
                    caches.mechtron_configs.add( self.root_caches.mechtron_configs.get(artifact).await? );
                }
                ArtifactKind::BindConfig => {
                    caches.bind_configs.add( self.root_caches.bind_configs.get(artifact).await? );
                }
                ArtifactKind::Wasm=> {
                    caches.wasms.add( self.root_caches.wasms.get(artifact).await? );
                }
/*                ArtifactKind::HttpRouter => {
                    caches.http_router_config.add( self.root_caches.http_router_configs.get(artifact).await? );
                }

 */
            }
        }

        Ok(caches)
    }
}


enum ProtoArtifactCall {
    Cache {
        artifacts: Vec<ArtifactRef>,
        tx: oneshot::Sender<Result<(), Error>>,
    },
}

impl Call for ProtoArtifactCall {}

struct ProtoArtifactCacheProc {
    proc_tx: mpsc::Sender<ProtoArtifactCall>,
    claims: AsyncHashMap<ArtifactRef, Claim>,
    root_caches: Arc<RootArtifactCaches>,
}

impl ProtoArtifactCacheProc {
    fn new(
        root_caches: Arc<RootArtifactCaches>,
        claims: AsyncHashMap<ArtifactRef, Claim>,
        proc_tx: mpsc::Sender<ProtoArtifactCall>,
    ) -> Self {
        ProtoArtifactCacheProc {
            proc_tx,
            root_caches,
            claims,
        }
    }

    async fn cache(
        proc_tx: mpsc::Sender<ProtoArtifactCall>,
        root_caches: Arc<RootArtifactCaches>,
        claims: AsyncHashMap<ArtifactRef, Claim>,
        artifacts: Vec<ArtifactRef>,
    ) -> Result<(), Error> {
        let mut more = vec![];

        for artifact in artifacts {
            println!(".... CACHING... {}", artifact.clone().address.to_string());
            let claim = root_caches.claim(artifact).await?;
            println!("claimed...");
            let references = claim.references();
            claims.put(claim.artifact.clone(), claim).await?;
            println!("put...");
            for reference in references {
                if !claims.contains(reference.clone()).await? {
                    more.push(reference);
                }
            }
            println!("processed artifact...");
        }
        println!("more is_empty(): {}", more.is_empty());
        if !more.is_empty() {
            let (sub_tx, sub_rx) = oneshot::channel();
            proc_tx
                .send(ProtoArtifactCall::Cache {
                    artifacts: more,
                    tx: sub_tx,
                })
                .await;
            sub_rx.await??;
            Ok(())
        } else {
            Ok(())
        }
    }
}

#[async_trait]
impl AsyncProcessor<ProtoArtifactCall> for ProtoArtifactCacheProc {
    async fn process(&mut self, call: ProtoArtifactCall) {
        match call {
            ProtoArtifactCall::Cache { artifacts, tx } => {
                let proc_tx = self.proc_tx.clone();
                let root_caches = self.root_caches.clone();
                let claims = self.claims.clone();
                tokio::spawn(async move {
                    tx.send(
                        ProtoArtifactCacheProc::cache(
                            proc_tx.clone(),
                            root_caches.clone(),
                            claims.clone(),
                            artifacts,
                        )
                        .await,
                    );
                });
            }
        }
    }
}

pub enum ArtifactBundleCacheCommand {
    Cache {
        bundle: Address,
        tx: oneshot::Sender<Result<(), Error>>,
    },
    Result {
        bundle: Address,
        result: Result<(), Error>,
    },
}

struct ArtifactBundleCacheRunner {
    tx: tokio::sync::mpsc::Sender<ArtifactBundleCacheCommand>,
    rx: tokio::sync::mpsc::Receiver<ArtifactBundleCacheCommand>,
    src: ArtifactBundleSrc,
    file_access: FileAccess,
    notify: HashMap<Address, Vec<oneshot::Sender<Result<(), Error>>>>,
    logger: AuditLogger,
    machine: StarlaneMachine
}

impl ArtifactBundleCacheRunner {
    pub fn new(
        src: ArtifactBundleSrc,
        file_access: FileAccess,
        machine: StarlaneMachine,
        logger: AuditLogger,
    ) -> tokio::sync::mpsc::Sender<ArtifactBundleCacheCommand> {
        let (tx, rx) = tokio::sync::mpsc::channel(1024);
        let runner = ArtifactBundleCacheRunner {
            file_access: file_access,
            src: src,
            rx: rx,
            tx: tx.clone(),
            notify: HashMap::new(),
            machine,
            logger: logger,
        };
        thread::spawn(move || {
            let mut builder = tokio::runtime::Builder::new_current_thread();
            builder.enable_all();
            let rt = builder
                .build()
                .expect("<ArtifactBundleCacheRunner> FATAL: could not get tokio runtime");
            rt.block_on(async move {
                runner.run().await;
            });
        });
        tx
    }

    async fn run(mut self) {
        while let Option::Some(command) = self.rx.recv().await {
            match command {
                ArtifactBundleCacheCommand::Cache { bundle, tx } => {
                    let bundle_address: Address = bundle.clone().into();
                    let record = match self.src.fetch_resource_record(bundle_address).await {
                        Ok(record) => record,
                        Err(err) => {
                            tx.send(Err(err.into()));
                            continue;
                        }
                    };

                        if self.has(bundle.clone()).await.is_ok() {
                        tx.send(Ok(()));
                    } else {
                        let first = if !self.notify.contains_key(&record.stub.address) {
                            self.notify.insert(record.stub.address.clone(), vec![]);
                            true
                        } else {
                            false
                        };

                        let notifiers = self.notify.get_mut(&record.stub.address ).unwrap();
                        notifiers.push(tx);

                        let src = self.src.clone();
                        let file_access = self.file_access.clone();
                        let tx = self.tx.clone();
                        if first {
                            let logger = self.logger.clone();
                            let machine = self.machine.clone();
                            tokio::spawn(async move {
                                let result = Self::download_and_extract(
                                    src,
                                    file_access,
                                    bundle.clone(),
                                    machine,
                                    logger,
                                )
                                .await;
                                tx.send(ArtifactBundleCacheCommand::Result {
                                    bundle: record.stub.address.clone(),
                                    result: result,
                                })
                                .await;
                            });
                        }
                    }
                }
                ArtifactBundleCacheCommand::Result { bundle, result } => {
                    let notifiers = self.notify.remove(&bundle);
                    if let Option::Some(mut notifiers) = notifiers {
                        for notifier in notifiers.drain(..) {
                            match &result {
                                Ok(_) => {
                                    notifier.send(Ok(()));
                                }
                                Err(error) => {
                                    notifier.send(Err(error.clone()));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    async fn has(&self, bundle: Address ) -> Result<(), Error> {
        let file_access =
            ArtifactBundleCache::with_bundle_path(self.file_access.clone(), bundle.clone())?;
        file_access.read(&Path::from_str("/.ready")?).await?;
        Ok(())
    }

    async fn download_and_extract(
        src: ArtifactBundleSrc,
        file_access: FileAccess,
        bundle: Address,
        machine: StarlaneMachine,
        logger: AuditLogger,
    ) -> Result<(), Error> {
        let bundle: Address = bundle.into();
        println!("download&extract src.fetch_resource_record...");
        let record = src.fetch_resource_record(bundle.clone()).await?;
        println!("download&extract src.fetch_resource_record DONE");

        let zip = src.get_bundle_zip(bundle.clone()).await?;


println!("Pre FileAccess");
        let mut file_access =
            ArtifactBundleCache::with_bundle_path(file_access, record.stub.address.clone().try_into()?)?;
        let bundle_zip_path = Path::from_str("/bundle.zip")?;
        let key_file = Path::from_str("/key.ser")?;
        file_access.write(
            &key_file,
            Arc::new(record.stub.address.to_string().as_bytes().to_vec()),
        );

println!("WRITING...{}", bundle_zip_path.to_string());
        file_access.write(&bundle_zip_path, zip).await?;
println!("DONE WRITING...");

println!("extracting files...");
        file_access
            .unzip("bundle.zip".to_string(), "files".to_string())
            .await?;
println!("done extracting files......");

        let ready_file = Path::from_str("/.ready")?;
        file_access
            .write(
                &ready_file,
                Arc::new("READY".to_string().as_bytes().to_vec()),
            )
            .await?;

        logger.log(Audit::Download(bundle.try_into()?));
println!("cache DONE");

        Ok(())
    }
}

#[derive(Clone)]
pub struct ArtifactBundleCache {
    file_access: FileAccess,
    tx: tokio::sync::mpsc::Sender<ArtifactBundleCacheCommand>,
}

impl ArtifactBundleCache {
    pub fn new(
        src: ArtifactBundleSrc,
        file_access: FileAccess,
        machine: StarlaneMachine,
        logger: AuditLogger,
    ) -> Result<Self, Error> {
        let tx = ArtifactBundleCacheRunner::new(src, file_access.clone(), machine, logger);
        Ok(ArtifactBundleCache {
            file_access: file_access,
            tx: tx,
        })
    }

    pub async fn download(&self, bundle: Address) -> Result<(), Error> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(ArtifactBundleCacheCommand::Cache { bundle, tx })
            .await;
        rx.await?
    }

    pub fn file_access(&self) -> FileAccess {
        self.file_access.clone()
    }

    pub fn with_bundle_files_path(
        file_access: FileAccess,
        address: Address,
    ) -> Result<FileAccess, Error> {
        Ok(file_access.with_path(format!("bundles/{}/files", address.to_string()))?)
    }

    pub fn with_bundle_path(
        file_access: FileAccess,
        address: Address,
    ) -> Result<FileAccess, Error> {
        Ok(file_access.with_path(format!("bundles/{}", address.to_string()))?)
    }
}

#[derive(Clone)]
pub enum ArtifactBundleSrc {
    STARLANE_API(StarlaneApi),
}

impl ArtifactBundleSrc {
    pub async fn get_bundle_zip(
        &self,
        address: Address,
    ) -> Result<Bin, Error> {
        Ok(match self {
            ArtifactBundleSrc::STARLANE_API(api) => {
                                let bundle: Primitive = api.get_state(address).await?.try_into()?;
                                 bundle.try_into()?
            }
            //            ArtifactBundleSrc::MOCK(mock) => mock.get_resource_state(address).await,
        })
    }

    pub async fn fetch_resource_record(
        &self,
        address: Address,
    ) -> Result<ResourceRecord, Error> {
        match self {
            ArtifactBundleSrc::STARLANE_API(api) => api.fetch_resource_record(address).await,
            //            ArtifactBundleSrc::MOCK(mock) => mock.fetch_resource_record(address).await,
        }
    }
}

impl From<StarlaneApi> for ArtifactBundleSrc {
    fn from(api: StarlaneApi) -> Self {
        ArtifactBundleSrc::STARLANE_API(api)
    }
}

/*
impl From<MockArtifactBundleSrc> for ArtifactBundleSrc {
    fn from(mock: MockArtifactBundleSrc) -> Self {
        ArtifactBundleSrc::MOCK(mock)
    }
}

#[derive(Clone)]
pub struct MockArtifactBundleSrc {
    pub resource: ResourceRecord,
}

impl MockArtifactBundleSrc {
    pub fn new() -> Result<Self, Error> {
        let key = ResourceKey::ArtifactBundle(ArtifactBundleKey {
            sub_space: SubSpaceKey {
                space: SpaceKey::new(RootKey{},0),
                id: 0,
            },
            id: 0,
        });

        let address = ResourceAddress::from_str("hyperspace:default:whiz:1.0.0::<ArtifactBundle>")?;

        Ok(MockArtifactBundleSrc {
            resource: ResourceRecord {
                stub: ResourceStub {
                    key: key,
                    address: address,
                    archetype: ResourceArchetype {
                        kind: ResourceKind::ArtifactBundle(ArtifactBundleKind::Final),
                        specific: None,
                        config: None,
                    },
                    owner: None,
                },
                location: ResourceLocation {
                    shell: StarKey::central(),
                },
            },
        })
    }
}

impl MockArtifactBundleSrc {
    pub async fn get_resource_state(
        &self,
        address: Address,
    ) -> Result<Option<Arc<Vec<u8>>>, Fail> {
        let mut file = fs::File::open("test-data/localhost-config/artifact-bundle.zip").await?;
        let mut data = vec![];
        file.read_to_end(&mut data).await?;
        Ok(Option::Some(Arc::new(data)))
    }

    pub async fn fetch_resource_record(
        &self,
        address: Address,
    ) -> Result<ResourceRecord, Fail> {
        Ok(self.resource.clone())
    }
}

 */

pub struct RefCount<C: Cacheable> {
    pub count: usize,
    pub reference: Arc<C>,
}

impl<C: Cacheable> RefCount<C> {
    pub fn new(reference: Arc<C>) -> Self {
        RefCount {
            count: 0,
            reference: reference,
        }
    }

    pub fn inc(&mut self) {
        self.count = self.count + 1;
    }

    pub fn dec(&mut self) {
        self.count = self.count - 1;
    }
}

pub struct RootItemCache<C: Cacheable> {
    tx: mpsc::Sender<RootItemCacheCall<C>>,
}

impl<C: Cacheable> RootItemCache<C> {
    pub fn new(bundle_cache: ArtifactBundleCache, parser: Arc<dyn Parser<C>>) -> Self {
        let (tx, rx) = mpsc::channel(256);

        AsyncRunner::new(
            Box::new(RootItemCacheProc::new(bundle_cache, parser, tx.clone())),
            tx.clone(),
            rx,
        );

        Self { tx: tx }
    }

    pub async fn cache(&self, artifact: ArtifactRef) -> Result<ArtifactItem<C>, Error> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(RootItemCacheCall::Cache { artifact, tx })
            .await?;
        rx.await?
    }

    pub async fn get(&self, artifact: ArtifactRef) -> Result<ArtifactItem<C>, Error> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(RootItemCacheCall::Get { artifact, tx })
            .await?;
        rx.await?
    }
}

impl<C: Cacheable> Call for RootItemCacheCall<C> {}

pub struct ArtifactItem<C: Cacheable> {
    item: Arc<C>,
    ref_tx: mpsc::Sender<RootItemCacheCall<C>>,
}

impl<C: Cacheable> ArtifactItem<C> {
    fn new(item: Arc<C>, ref_tx: mpsc::Sender<RootItemCacheCall<C>>) -> Self {
        let ref_tx_cp = ref_tx.clone();
        let item_cp = item.clone();
        tokio::spawn(async move {
            ref_tx_cp
                .send(RootItemCacheCall::Increment {
                    artifact: item_cp.artifact(),
                    item: item_cp,
                })
                .await;
        });
        ArtifactItem {
            item: item,
            ref_tx: ref_tx,
        }
    }
}

impl<C: Cacheable> Deref for ArtifactItem<C> {
    type Target = Arc<C>;

    fn deref(&self) -> &Self::Target {
        &self.item
    }
}

impl<C: Cacheable> Clone for ArtifactItem<C> {
    fn clone(&self) -> Self {
        ArtifactItem::new(self.item.clone(), self.ref_tx.clone())
    }
}

enum ClaimCall {
    Increment,
    Decrement,
}

impl<C: Cacheable> Into<Claim> for ArtifactItem<C> {
    fn into(self) -> Claim {
        let (tx, mut rx) = mpsc::channel(1);
        let item = self.item.clone();
        let ref_tx = self.ref_tx.clone();
        tokio::spawn(async move {
            while let Option::Some(call) = rx.recv().await {
                match call {
                    ClaimCall::Increment => {
                        ref_tx
                            .send(RootItemCacheCall::Increment {
                                artifact: item.artifact(),
                                item: item.clone(),
                            })
                            .await;
                    }
                    ClaimCall::Decrement => {
                        ref_tx
                            .send(RootItemCacheCall::Decrement(item.artifact()))
                            .await;
                    }
                }
            }
        });

        Claim::new(self.item.artifact(), self.item.references(), tx)
    }
}

impl Clone for Claim {
    fn clone(&self) -> Self {
        Self::new(
            self.artifact.clone(),
            self.references.clone(),
            self.ref_tx.clone(),
        )
    }
}

struct Claim {
    artifact: ArtifactRef,
    references: Vec<ArtifactRef>,
    ref_tx: mpsc::Sender<ClaimCall>,
}

impl Claim {
    fn new(
        artifact: ArtifactRef,
        references: Vec<ArtifactRef>,
        ref_tx: mpsc::Sender<ClaimCall>,
    ) -> Self {
        let ref_tx_cp = ref_tx.clone();

        tokio::spawn(async move {
            ref_tx_cp.send(ClaimCall::Increment).await;
        });

        Self {
            artifact: artifact,
            references: references,
            ref_tx: ref_tx,
        }
    }
}

impl Cacheable for Claim {
    fn artifact(&self) -> ArtifactRef {
        self.artifact.clone()
    }

    fn references(&self) -> Vec<ArtifactRef> {
        self.references.clone()
    }
}

impl Drop for Claim {
    fn drop(&mut self) {
        let ref_tx = self.ref_tx.clone();
        tokio::spawn(async move {
            ref_tx.send(ClaimCall::Decrement).await;
        });
    }
}

pub enum RootItemCacheCall<C: Cacheable> {
    Cache {
        artifact: ArtifactRef,
        tx: oneshot::Sender<Result<ArtifactItem<C>, Error>>,
    },
    Get {
        artifact: ArtifactRef,
        tx: oneshot::Sender<Result<ArtifactItem<C>, Error>>,
    },
    Increment {
        artifact: ArtifactRef,
        item: Arc<C>,
    },
    Decrement(ArtifactRef),
    Signal {
        artifact: ArtifactRef,
        result: Result<ArtifactItem<C>, Error>,
    },
}

struct RootItemCacheProc<C: Cacheable> {
    bundle_cache: ArtifactBundleCache,
    map: HashMap<ArtifactRef, RefCount<C>>,
    signal_map: HashMap<ArtifactRef, Vec<oneshot::Sender<Result<ArtifactItem<C>, Error>>>>,
    parser: Arc<dyn Parser<C>>,
    proc_tx: mpsc::Sender<RootItemCacheCall<C>>,
}

impl<C: Cacheable> RootItemCacheProc<C> {
    pub fn new(
        bundle_cache: ArtifactBundleCache,
        parser: Arc<dyn Parser<C>>,
        proc_tx: mpsc::Sender<RootItemCacheCall<C>>,
    ) -> Self {
        RootItemCacheProc {
            bundle_cache: bundle_cache,
            map: HashMap::new(),
            parser: parser,
            proc_tx: proc_tx,
            signal_map: HashMap::new(),
        }
    }
}

#[async_trait]
impl<C: Cacheable> AsyncProcessor<RootItemCacheCall<C>> for RootItemCacheProc<C> {
    async fn process(&mut self, call: RootItemCacheCall<C>) {
        match call {
            RootItemCacheCall::Increment { artifact, item } => {
                let ref_count = if self.map.contains_key(&artifact) {
                    self.map.get_mut(&artifact).unwrap()
                } else {
                    let ref_count = RefCount::new(item);
                    self.map.insert(artifact.clone(), ref_count);
                    self.map.get_mut(&artifact).unwrap()
                };
                ref_count.inc();
            }
            RootItemCacheCall::Decrement(artifact) => {
                let ref_count = self.map.get_mut(&artifact);
                match ref_count {
                    None => {}
                    Some(ref_count) => {
                        ref_count.dec();
                        if ref_count.count <= 0 {
                            self.map.remove(&artifact);
                        }
                    }
                }
            }
            RootItemCacheCall::Cache { artifact, tx } => {
                if self.map.contains_key(&artifact) {
                    let item = self.map.get(&artifact).unwrap().reference.clone();
                    tx.send(Ok(ArtifactItem::new(item, self.proc_tx.clone())));
                } else {
                    if self.signal_map.contains_key(&artifact) {
                        self.signal_map.get_mut(&artifact).unwrap().push(tx);
                    } else {
                        self.signal_map.insert(artifact.clone(), vec![tx]);
                    }

                    self.cache(artifact).await;
                }
            }
            RootItemCacheCall::Signal { artifact, result } => {
                if let Option::Some(txs) = self.signal_map.remove(&artifact) {
                    for tx in txs {
                        tx.send(result.clone());
                    }
                }
            }
            RootItemCacheCall::Get { artifact, tx } => {
                tx.send(self.get(artifact));
            }
        }
    }
}

impl<C: Cacheable> RootItemCacheProc<C> {
    fn get(&self, artifact: ArtifactRef) -> Result<ArtifactItem<C>, Error> {
        let ref_count = self.map.get(&artifact).ok_or(format!(
            "could not find artifact: '{}'",
            artifact.address.to_string()
        ))?;
        Ok(ArtifactItem::new(
            ref_count.reference.clone(),
            self.proc_tx.clone(),
        ))
    }

    async fn cache(&self, artifact: ArtifactRef) {
        let parser = self.parser.clone();
        let bundle_cache = self.bundle_cache.clone();
        let proc_tx = self.proc_tx.clone();

        tokio::spawn(async move {
            match Self::cache_artifact(artifact.clone(), parser.clone(), bundle_cache.clone()).await
            {
                Ok(item) => {
                    proc_tx
                        .send(RootItemCacheCall::Signal {
                            artifact,
                            result: Ok(ArtifactItem::new(item, proc_tx.clone())),
                        })
                        .await;
                }
                Err(err) => {
                    proc_tx
                        .send(RootItemCacheCall::Signal {
                            artifact,
                            result: Err(err.into()),
                        })
                        .await;
                }
            }
        });
    }

    async fn cache_artifact<X: Cacheable>(
        artifact: ArtifactRef,
        parser: Arc<dyn Parser<X>>,
        bundle_cache: ArtifactBundleCache,
    ) -> Result<Arc<X>, Error> {
        println!("root: cache_artifact: {}", artifact.address.to_string());
        let address: Address = artifact.address.parent().ok_or("expected parent for artifact")?.into();
        bundle_cache.download(address.try_into()?).await?;
        println!("bundle cached : parsing: {}", artifact.address.to_string());
        let file_access = ArtifactBundleCache::with_bundle_files_path(
            bundle_cache.file_access(),
            artifact.address.parent().ok_or("expected parent")?.try_into()?,
        )?;
        println!(
            "file acces scached : parsing: {}",
            artifact.address.to_string()
        );
        let data = file_access.read(&artifact.trailing_path()?).await?;
        println!("root: parsing: {}", artifact.address.to_string());
        parser.parse(artifact, data)
    }
}

struct RootArtifactCaches {
    bundle_cache: ArtifactBundleCache,
    raw: RootItemCache<Raw>,
    app_configs: RootItemCache<AppConfig>,
    mechtron_configs: RootItemCache<MechtronConfig>,
    bind_configs: RootItemCache<BindConfig>,
    wasms: RootItemCache<Wasm>,
//    http_router_configs: RootItemCache<HttpRouterConfig>
}

impl RootArtifactCaches {
    fn new(bundle_cache: ArtifactBundleCache) -> Self {

        Self {
            bundle_cache: bundle_cache.clone(),
            raw: RootItemCache::new(bundle_cache.clone(), Arc::new(RawParser::new())),
            app_configs: RootItemCache::new(bundle_cache.clone(), Arc::new(AppConfigParser::new())),
            mechtron_configs: RootItemCache::new(bundle_cache.clone(), Arc::new(MechtronConfigParser::new())),
            bind_configs: RootItemCache::new(bundle_cache.clone(), Arc::new(BindConfigParser::new())),
            wasms: RootItemCache::new(bundle_cache.clone(), Arc::new(WasmCompiler::new())),
//            http_router_configs: RootItemCache::new(bundle_cache.clone(), Arc::new(HttpRouterConfigParser::new())),

        }
    }

    async fn claim(&self, artifact: ArtifactRef) -> Result<Claim, Error> {
        let claim = match artifact.kind {
            ArtifactKind::AppConfig=> self.app_configs.cache(artifact).await?.into(),
            ArtifactKind::MechtronConfig=> self.mechtron_configs.cache(artifact).await?.into(),
            ArtifactKind::BindConfig=> self.bind_configs.cache(artifact).await?.into(),
            ArtifactKind::Raw => self.raw.cache(artifact).await?.into(),
            ArtifactKind::Wasm=> self.wasms.cache(artifact).await?.into(),
//            ArtifactKind::HttpRouter => self.http_router_configs.cache(artifact).await?.into(),
        };

        Ok(claim)
    }
}

#[derive(Clone)]
pub enum Audit {
    Download(Address),
}

#[derive(Clone)]
pub struct AuditLogger {
    sender: broadcast::Sender<Audit>,
}

impl AuditLogger {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(16);
        Self { sender }
    }

    pub fn collector(&self) -> AuditLogCollector {
        AuditLogCollector::new(self.sender.subscribe())
    }

    pub fn log(&self, log: Audit) {
        self.sender.send(log);
    }
}

pub struct AuditLogCollector {
    tx: mpsc::Sender<AuditLogCollectorCall>,
}

impl AuditLogCollector {
    pub fn new(receiver: broadcast::Receiver<Audit>) -> Self {
        AuditLogCollector {
            tx: AuditLogCollectorProc::new(receiver),
        }
    }
}

struct AuditLogCollectorProc {
    receiver: broadcast::Receiver<Audit>,
    vec: Vec<Audit>,
    tx: mpsc::Sender<AuditLogCollectorCall>,
    rx: mpsc::Receiver<AuditLogCollectorCall>,
}

enum AuditLogCollectorCall {
    Get(oneshot::Sender<Vec<Audit>>),
    Log(Audit),
}

impl AuditLogCollectorProc {
    pub fn new(receiver: broadcast::Receiver<Audit>) -> mpsc::Sender<AuditLogCollectorCall> {
        let (tx, rx) = mpsc::channel(1);

        let proc = AuditLogCollectorProc {
            receiver,
            vec: vec![],
            tx: tx.clone(),
            rx,
        };

        proc.run();

        tx
    }

    pub fn run(self) {
        let handle = Handle::current();

        let tx = self.tx;
        let mut receiver = self.receiver;
        let mut vec = self.vec;
        let mut rx = self.rx;

        handle.spawn(async move {
            while let Result::Ok(audit) = receiver.recv().await {
                tx.send(AuditLogCollectorCall::Log(audit)).await;
            }
        });

        handle.spawn(async move {
            while let Option::Some(call) = rx.recv().await {
                match call {
                    AuditLogCollectorCall::Get(tx) => {
                        tx.send(vec.clone());
                    }
                    AuditLogCollectorCall::Log(log) => {
                        vec.push(log);
                    }
                }
            }
        });
    }
}

/*
#[cfg(test)]
mod test {
    use std::fs;
    use std::str::FromStr;

    use tokio::runtime::Runtime;
    use tokio::time::{Duration, sleep};

    use crate::artifact::ArtifactRef;
    use crate::cache::{
        ArtifactBundleCache, ArtifactBundleSrc, AuditLogger, MockArtifactBundleSrc,
        ProtoArtifactCachesFactory, RootArtifactCaches, RootItemCache,
    };
    use crate::error::Error;
    use crate::file_access::FileAccess;
    use crate::resource::ArtifactAddress;
    use crate::resource::ArtifactBundleAddress;
    use crate::resource::ArtifactKind;
    use crate::resource::RootKey;

    fn reset() {
        let data_dir = "tmp/data";
        let cache_dir = "tmp/cache";
        fs::remove_dir_all(data_dir).unwrap_or_default();
        fs::remove_dir_all(cache_dir).unwrap_or_default();
        std::env::set_var("STARLANE_DATA", data_dir);
        std::env::set_var("STARLANE_CACHE", cache_dir);
    }

    #[test]
    pub fn some_test() -> Result<(), Error> {
        let mut builder = tokio::runtime::Builder::new_multi_thread();
        let rt = builder.enable_time().enable_io().enable_all().build()?;

        rt.block_on(async {
            reset();
            assert!(async_bundle_test().await.is_ok());
            reset();
            assert!(root_item_cache_test().await.is_ok());
            reset();
            assert!(proto_caches().await.is_ok());
        });

        Ok(())
    }



    pub async fn proto_caches() -> Result<(), Error> {
        let factory = ProtoArtifactCachesFactory::new(
            MockArtifactBundleSrc::new()?.into(),
            FileAccess::new("tmp/cache".to_string())?,
        )?;

        let mut proto_caches = factory.create();
        let artifact = ArtifactAddress::from_str("hyperspace:default:whiz:1.0.0:/routes.txt")?;

        let artifact = ArtifactRef::new(artifact, ArtifactKind::DomainConfig );

        proto_caches.cache(vec![artifact.clone()]).await?;

        let caches = proto_caches.to_caches().await?;

        let domain_config = caches.domain_configs.get(&artifact.address).ok_or(format!(
            "expected address '{}'",
            artifact.address.to_string()
        ))?;

        Ok(())
    }

    pub async fn root_item_cache_test() -> Result<(), Error> {
        let bundle_cache = ArtifactBundleCache::new(
            MockArtifactBundleSrc::new()?.into(),
            FileAccess::new("tmp/cache".to_string())?,
            AuditLogger::new(),
        )?;
        let artifact = ArtifactAddress::from_str("hyperspace:default:whiz:1.0.0:/routes.txt")?;
        let artifact = ArtifactRef {
            address: artifact,
            kind: ArtifactKind::Raw,
        };

        let root_caches = RootArtifactCaches::new(bundle_cache);

        let rtn = root_caches.raw.cache(artifact).await;
        assert!(rtn.is_ok());

        let raw = rtn.expect("expeted to get raw data");
        let raw = String::from_utf8((*raw.data()).clone() )?;

        println!("RAW {}", raw );


        //      tokio::time::sleep( Duration::from_secs(5)).await;

        Ok(())
    }

    pub async fn async_bundle_test() -> Result<(), Error> {
        let bundle_cache = ArtifactBundleCache::new(
            MockArtifactBundleSrc::new()?.into(),
            FileAccess::new("tmp/cache".to_string())?,
            AuditLogger::new(),
        )?;
        let bundle = ArtifactBundleAddress::from_str("hyperspace:default:whiz:1.0.0")?;

        // make sure the files aren't there NOW.
        assert!(
            fs::File::open("tmp/cache/bundles/hyperspace:default:whiz:1.0.0/bundle.zip").is_err()
        );
        assert!(fs::File::open("tmp/cache/bundles/hyperspace:default:whiz:1.0.0/.ready").is_err());
        assert!(
            fs::File::open("tmp/cache/bundles/hyperspace:default:whiz:1.0.0/files/routes.txt")
                .is_err()
        );

        bundle_cache.download(bundle.into()).await?;

        // here we should verify that the correct files were created.
        assert!(
            fs::File::open("tmp/cache/bundles/hyperspace:default:whiz:1.0.0/bundle.zip").is_ok()
        );
        assert!(fs::File::open("tmp/cache/bundles/hyperspace:default:whiz:1.0.0/.ready").is_ok());
        assert!(
            fs::File::open("tmp/cache/bundles/hyperspace:default:whiz:1.0.0/files/routes.txt")
                .is_ok()
        );

        Ok(())
    }
}

 */

pub struct Raw {
    data: Data,
    artifact: ArtifactRef,
}

impl Raw {
    pub fn data(&self) -> Data {
        self.data.clone()
    }
}

impl Cacheable for Raw {
    fn artifact(&self) -> ArtifactRef {
        self.artifact.clone()
    }

    fn references(&self) -> Vec<ArtifactRef> {
        vec![]
    }
}

pub struct RawParser;

impl RawParser {
    pub fn new() -> Self {
        Self {}
    }
}

impl Parser<Raw> for RawParser {
    fn parse(&self, artifact: ArtifactRef, data: Data) -> Result<Arc<Raw>, Error> {
        Ok(Arc::new(Raw {
            artifact: artifact,
            data: data,
        }))
    }
}
