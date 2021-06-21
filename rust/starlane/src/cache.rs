use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::thread;

use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::runtime::{Handle, Runtime};
use tokio::sync::oneshot::Receiver;
use tokio::sync::{broadcast, mpsc, oneshot};

use crate::artifact::{ArtifactAddress, ArtifactBundleAddress, ArtifactBundleId, ArtifactBundleIdentifier, ArtifactBundleKey, ArtifactIdentifier, ArtifactKey, ArtifactRef, ArtifactKind};
use crate::error::Error;
use crate::file_access::FileAccess;
use crate::keys::{ResourceId, ResourceKey, SpaceKey, SubSpaceKey};
use crate::logger::{elog, LogInfo, StaticLogInfo};
use crate::message::Fail;
use crate::resource::artifact::ArtifactBundle;
use crate::resource::config::{FromArtifact, Parser};
use crate::resource::domain::{DomainConfig, DomainConfigParser};
use crate::resource::{
    ArtifactBundleKind, Path, ResourceAddress, ResourceArchetype, ResourceIdentifier, ResourceKind,
    ResourceLocation, ResourceRecord, ResourceStub,
};
use crate::star::{StarCommand, StarKey};
use crate::starlane::api::StarlaneApi;
use crate::util::{AsyncHashMap, AsyncProcessor, AsyncRunner, Call};
use actix_web::error::Canceled;
use std::collections::hash_set::Difference;
use std::convert::TryInto;
use std::future::Future;
use std::iter::FromIterator;
use std::ops::Deref;
use std::str::FromStr;
use tokio::fs;
use tokio::sync::oneshot::error::RecvError;
use std::collections::hash_map::RandomState;
use std::hash::{Hash, Hasher};

pub type Data = Arc<Vec<u8>>;
pub type ZipFile = Path;

#[derive(Clone)]
pub struct ProtoCacheFactory {
    logger: AuditLogger,
    cache_call_tx: mpsc::Sender<CacheCall>,
    root_caches: RootCache,
}

impl ProtoCacheFactory {
    pub fn new(src: ArtifactBundleSrc, file_access: FileAccess) -> Result<ProtoCacheFactory, Error> {
        let logger = AuditLogger::new();
        let bundle_cache = ArtifactBundleCache::new(src, file_access.clone(), logger.clone())?;
        let root_caches = RootCache {
            item_caches: RootItemCaches { domain_configs: Arc::new(()) }
        };

        let (cache_call_tx, cache_call_rx) = mpsc::channel(16 * 1024);
        AsyncRunner::new(
            Box::new(ProtoCachesFactoryProc {
                tx: cache_call_tx.clone(),
                bundle_cache: bundle_cache.clone(),
                root_caches: root_caches.clone(),
            }),
            cache_call_tx.clone(),
            cache_call_rx,
        );

        Ok(Self {
            root_caches,
            logger: logger,
            cache_call_tx: cache_call_tx,
        })
    }

    pub fn create(&self) -> ProtoCache {
        ProtoCache::new(self.cache_call_tx.clone())
    }
}

#[derive(Clone)]
pub struct ProtoCache {
    map: AsyncHashMap<ArtifactRef,Result<Claim,Error>>,
    cache_call_tx: mpsc::Sender<CacheCall>
}

impl ProtoCache {
    fn new(cache_call_tx: mpsc::Sender<CacheCall>) -> Self {
        ProtoCache{
            map: AsyncHashMap::new(),
            cache_call_tx: cache_call_tx
        }
    }

    pub async fn wait_cache(&self, artifact: ArtifactRef) -> Result<(), Error> {
        let rx = self.cache(artifact);
        rx.await?
    }

    pub fn cache(&self, artifact: ArtifactRef) -> oneshot::Receiver<Result<(), Error>> {
        let (tx, rx) = oneshot::channel();
        let cache_call_tx = self.cache_call_tx.clone();
        let map = self.map.clone();
        tokio::spawn(async move {
            let (sub_tx,sub_rx) = oneshot::channel();
            cache_call_tx.send(CacheCall::Cache { artifact: artifact.clone(), tx: sub_tx }).await;
            map.put( artifact, result.clone() ).await;
            let result = match sub_rx.await
            {
                Ok(result) => {
                    match result {
                        Ok(claim) => {
                            Ok(())
                        }
                        Err(error) => {
                            Err(error)
                        }
                    }
                }
                Err(err) => {
                    eprintln!("ProtoCache: RecvError when waiting for cache artifact: {:?}", artifact );
                    Err(format!("ProtoCache: RecvError when waiting for cache artifact: {:?}", artifact).into())
                }
            };
            tx.send(result);
        });
        rx
    }
}


#[derive(Clone)]
pub struct RootItemCaches{
    domain_configs: Arc<RootItemCache<DomainConfig>>,
}

#[derive(Clone)]
pub struct RootCache {
    item_caches: RootItemCaches
}

impl RootCache {


    async fn cache(
        &mut self,
        artifacts: Vec<ArtifactRef>,
        tx: oneshot::Sender<Result<HashSet<Claim>, Error>>,
    ) {
        let mut cache_artifacts = artifacts.clone();
        // these are the ones we don't have in our cache yet
        cache_artifacts.retain(|artifact| !self.map.contains_key(artifact));

        // first we collect the artifacts we have already cached
        let mut rtn_artifacts = artifacts.clone();
        rtn_artifacts.retain(|artifact| self.map.contains_key(artifact));
        let rtn_artifacts = HashSet::from_iter(rtn_artifacts);

        let item_caches = self.item_caches.clone();
        let bundle_cache = self.bundle_cache.clone();

        tokio::spawn(async move {
            let mut futures = vec![];
            for artifact in cache_artifacts.clone() {
                futures.push(Self::cache_artifact(
                    artifact,
                    &bundle_cache,
                    &item_caches,
                ));
            }
            let results = futures::future::join_all(futures).await;
            for result in results {
                match result {
                    Ok(cached) => {
                        let mut cached = cached.iter().collect();
                        rtn_artifacts.append(&mut cached );
                    }
                    Err(err) => {
                        tx.send(Err(result.err().unwrap()));
                        return;
                    }
                }
            }
            tx.send(Ok(rtn_artifacts));
        });
    }

    async fn cache_artifact(
        artifact: ArtifactRef,
        bundle_cache: &ArtifactBundleCache,
        caches: &RootItemCaches
    ) -> Result<HashSet<ArtifactRef>, Error> {
        let bundle = artifact.parent();
        bundle_cache.download(bundle.clone().into()).await?;

        let rx = match artifact.kind {
            ArtifactKind::DomainConfig => {
                caches.domain_configs.cache( vec![artifact] )
            }
        };

        let mut refs = rx.await??;
        refs.push(artifact);
        let rtn = HashSet::from_iter(refs);

        Ok(rtn)
    }

}





enum CacheCall {
    Cache {
        artifact: ArtifactRef,
        tx: oneshot::Sender<Result<Claim, Error>>,
    },
}

struct ProtoCachesFactoryProc {
    tx: mpsc::Sender<CacheCall>,
    bundle_cache: ArtifactBundleCache,
    root_caches: RootCache,
}

impl ProtoCachesFactoryProc {
    fn cache(bundle_cache: ArtifactBundleCache, artifact_ref: ArtifactRef) -> Result<(), Error> {
        let bundle = artifact.artifact.parent();
        bundle_cache.download(bundle.into()).await?;
        Ok(())
    }
}

impl AsyncProcessor<CacheCall> for ProtoCachesFactoryProc {
    async fn process(&mut self, call: CacheCall) {
        match call {
            CacheCall::Cache { artifact, tx } => {
                let bundle_cache = self.bundle_cache.clone();
                tokio::spawn(async move {
                    tx.send(Self::cache(bundle_cache, artifact));
                });
            }
        }
    }
}

pub enum ArtifactBundleCacheCommand {
    Cache {
        bundle: ArtifactBundleIdentifier,
        tx: oneshot::Sender<Result<(), Error>>,
    },
    Result {
        bundle: ArtifactBundleAddress,
        result: Result<(), Error>,
    },
}

pub struct ArtifactBundleCacheRunner {
    tx: tokio::sync::mpsc::Sender<ArtifactBundleCacheCommand>,
    rx: tokio::sync::mpsc::Receiver<ArtifactBundleCacheCommand>,
    src: ArtifactBundleSrc,
    file_access: FileAccess,
    notify: HashMap<ArtifactBundleAddress, Vec<oneshot::Sender<Result<(), Error>>>>,
    logger: AuditLogger,
}

impl ArtifactBundleCacheRunner {
    pub fn new(
        src: ArtifactBundleSrc,
        file_access: FileAccess,
        logger: AuditLogger,
    ) -> tokio::sync::mpsc::Sender<ArtifactBundleCacheCommand> {
        let (tx, rx) = tokio::sync::mpsc::channel(1024);
        let runner = ArtifactBundleCacheRunner {
            file_access: file_access,
            src: src,
            rx: rx,
            tx: tx.clone(),
            notify: HashMap::new(),
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
                    let bundle: ResourceIdentifier = bundle.into();
                    let record = match self.src.fetch_resource_record(bundle.clone()).await {
                        Ok(record) => record,
                        Err(err) => {
                            tx.send(Err(err.into()));
                            continue;
                        }
                    };
                    let bundle: ArtifactBundleAddress = match record.stub.address.try_into() {
                        Ok(ok) => ok,
                        Err(err) => {
                            tx.send(Err(err.into()));
                            continue;
                        }
                    };

                    if self.has(bundle.clone()).await.is_ok() {
                        tx.send(Ok(()));
                    } else {
                        let first = if !self.notify.contains_key(&bundle) {
                            self.notify.insert(bundle.clone(), vec![]);
                            true
                        } else {
                            false
                        };

                        let notifiers = self.notify.get_mut(&bundle).unwrap();
                        notifiers.push(tx);

                        let src = self.src.clone();
                        let file_access = self.file_access.clone();
                        let tx = self.tx.clone();
                        if first {
                            let logger = self.logger.clone();
                            tokio::spawn(async move {
                                let result = Self::download_and_extract(
                                    src,
                                    file_access,
                                    bundle.clone(),
                                    logger,
                                )
                                .await;
                                tx.send(ArtifactBundleCacheCommand::Result {
                                    bundle: bundle.clone(),
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

    async fn has(&self, bundle: ArtifactBundleAddress) -> Result<(), Error> {
        let file_access =
            ArtifactBundleCache::with_bundle_path(self.file_access.clone(), bundle.clone()).await?;
        file_access.read(&Path::new("/.ready")?).await?;
        Ok(())
    }

    async fn download_and_extract(
        src: ArtifactBundleSrc,
        mut file_access: FileAccess,
        bundle: ArtifactBundleAddress,
        logger: AuditLogger,
    ) -> Result<(), Error> {
        let bundle: ResourceAddress = bundle.into();
        let bundle: ResourceIdentifier = bundle.into();
        let record = src.fetch_resource_record(bundle.clone()).await?;

        let stream = src
            .get_resource_state(bundle.clone())
            .await?
            .ok_or("expected bundle to have state")?;

        let mut file_access =
            ArtifactBundleCache::with_bundle_path(file_access, record.stub.address.try_into()?)
                .await?;
        let bundle_zip = Path::new("/bundle.zip")?;
        let key_file = Path::new("/key.ser")?;
        file_access.write(
            &key_file,
            Arc::new(record.stub.key.to_string().as_bytes().to_vec()),
        );
        file_access.write(&bundle_zip, stream).await?;

        file_access
            .unzip("bundle.zip".to_string(), "files".to_string())
            .await?;

        let ready_file = Path::new("/.ready")?;
        file_access
            .write(
                &ready_file,
                Arc::new("READY".to_string().as_bytes().to_vec()),
            )
            .await?;

        logger.log(Audit::Download(bundle.try_into()?));

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
        logger: AuditLogger,
    ) -> Result<Self, Error> {
        let tx = ArtifactBundleCacheRunner::new(src, file_access.clone(), logger);
        Ok(ArtifactBundleCache {
            file_access: file_access,
            tx: tx,
        })
    }

    pub async fn download(&self, bundle: ArtifactBundleIdentifier) -> Result<(), Error> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(ArtifactBundleCacheCommand::Cache { bundle, tx })
            .await;
        rx.await?
    }

    pub fn file_access(&self) -> FileAccess {
        self.file_access.clone()
    }

    pub async fn with_bundle_files_path(
        file_access: FileAccess,
        address: ArtifactBundleAddress,
    ) -> Result<FileAccess, Error> {
        let address: ResourceAddress = address.into();
        Ok(file_access.with_path(format!("bundles/{}/files", address.to_parts_string()))?)
    }

    pub async fn with_bundle_path(
        file_access: FileAccess,
        address: ArtifactBundleAddress,
    ) -> Result<FileAccess, Error> {
        let address: ResourceAddress = address.into();
        Ok(file_access.with_path(format!("bundles/{}", address.to_parts_string()))?)
    }
}




#[derive(Clone)]
pub struct ItemCache<C: Cacheable> {
    root: Arc<RootItemCache<C>>,
    map: HashMap<ArtifactAddress, Item<C>>,
}

impl<C: Cacheable> Into<OldProtoCache<C>> for ItemCache<C> {
    fn into(self) -> OldProtoCache<C> {
        OldProtoCache {
            map: AsyncHashMap::from(self.map),
            root: self.root,
        }
    }
}

impl<C: Cacheable> ItemCache<C> {
    pub fn get(&self, artifact: &ArtifactAddress) -> Result<Item<C>, Error> {
        let rtn = self.map.get(artifact).cloned();
        match rtn {
            None => {
                Err(format!("must call ProtoCache.cache('{}') for this artifact and wait for cache callback before get()",artifact.to_string()).into())
            }
            Some(cached) => {
                Ok(cached)
            }
        }
    }
}

#[derive(Clone)]
pub struct OldProtoCache<C: Cacheable> {
    root: Arc<RootItemCache<C>>,
    map: AsyncHashMap<ArtifactAddress, Item<C>>,
}

impl<C: Cacheable> OldProtoCache<C> {
    fn new(root: Arc<RootItemCache<C>>) -> Self {
        Self {
            root: root,
            map: AsyncHashMap::new(),
        }
    }

}

pub struct CacheFactory<C: Cacheable> {
    root_cache: Arc<RootItemCache<C>>,
}

impl<C: Cacheable> CacheFactory<C> {
    fn new(root_cache: Arc<RootItemCache<C>>) -> Self {
        Self {
            root_cache: root_cache,
        }
    }

    pub fn create(&self) -> OldProtoCache<C> {
        OldProtoCache::new(self.root_cache.clone())
    }
}

pub trait Cacheable: FromArtifact + Send + Sync + 'static {}


pub enum RootCacheCall {
    Cache {
        artifacts: Vec<ArtifactRef>,
        tx: oneshot::Sender<Result<Vec<Claim>, Error>>,
    },
    Increment(ArtifactRef),
    Decrement(ArtifactRef),
}

impl Call for RootCacheCall {}

pub struct Claim{
    pub artifact_ref: ArtifactRef,
    deref_tx: mpsc::Sender<RootCacheCall>,
}

impl Claim {
    pub fn new(artifact_ref: ArtifactRef, deref_tx: mpsc::Sender<RootCacheCall>) -> Self {
        let artifact_ref_cp= artifact_ref.clone();
        let deref_tx_cp = deref_tx.clone();
        tokio::spawn(async move {
            deref_tx_cp
                .send(RootCacheCall::Increment(artifact_ref_cp.artifact()) ).await;
        });

        Claim{
            artifact_ref: artifact_ref,
            deref_tx: deref_tx,
        }
    }
}


impl Drop for Claim {
    fn drop(&mut self) {
        self.deref_tx.send( RootCacheCall::Decrement(self.artifact_ref.clone()))
    }
}

pub struct Item<C: Cacheable> {
    item: Arc<C>,
    deref_tx: mpsc::Sender<RootCacheCall>,
}

impl <C> Into<Claim> for Item<C> {
    fn into(self) -> Claim {
        Claim ::new(self.item.artifact(), self.deref_tx.clone() )
    }
}

impl<C: Cacheable> Item<C> {
    pub fn new(item: Arc<C>, deref_tx: mpsc::Sender<RootCacheCall>) -> Self {
        let deref_tx_cp = deref_tx.clone();
        tokio::spawn(async move {
            deref_tx_cp
                .send(RootCacheCall::Increment(item.artifact())).await;
        });

        Item {
            item: item,
            deref_tx: deref_tx,
        }
    }
}

impl<C: Cacheable> Clone for Item<C> {
    fn clone(&self) -> Self {
        Self::new(self.item.clone(), self.deref_tx.clone())
    }
}

impl<C: Cacheable> Deref for Item<C> {
    type Target = Arc<C>;

    fn deref(&self) -> &Self::Target {
        &self.item
    }
}

impl<C: Cacheable> Drop for Item<C> {
    fn drop(&mut self) {
        self.deref_tx
            .send(RootCacheCall::Decrement(self.item.artifact()));
    }
}

struct RefCount<C> {
    pub count: usize,
    pub item: Arc<C>,
}

struct RootCacheProc<C: Cacheable> {
    pub tx: mpsc::Sender<RootCacheCall>,
    pub parser: Arc<dyn Parser<C>>,
    pub bundle_cache: ArtifactBundleCache,
    pub map: HashMap<ArtifactAddress, RefCount<C>>,
    pub file_access: FileAccess,
}

impl<C: Cacheable> RootCacheProc<C> {
    pub fn new(
        bundle_cache: ArtifactBundleCache,
        file_access: FileAccess,
        parser: Arc<dyn Parser<C>>,
    ) -> Result<mpsc::Sender<RootCacheCall>, Error> {
        let (tx, rx) = mpsc::channel(16);
        AsyncRunner::new(
            Box::new(RootCacheProc {
                tx: tx.clone(),
                parser: parser,
                file_access: file_access.clone(),
                bundle_cache: bundle_cache,
                map: HashMap::new(),
            }),
            tx.clone(),
            rx,
        );
        Ok(tx)
    }

    async fn cache(
        &mut self,
        artifacts: Vec<ArtifactRef>,
        tx: oneshot::Sender<Result<Vec<Item<C>>, Error>>,
    ) {
        let mut rtn = vec![];
        let mut fetch_artifacts = artifacts.clone();
        // these are the ones we don't have in our cache yet
        fetch_artifacts.retain(|artifact| !self.map.contains_key(artifact));

        {
            let artifacts: HashSet<ArtifactAddress> = HashSet::from_iter(artifacts);
            let fetch_artifacts: HashSet<ArtifactAddress> =
                HashSet::from_iter(fetch_artifacts.clone());
            let diff: Vec<ArtifactAddress> = artifacts
                .difference(&fetch_artifacts)
                .into_iter()
                .cloned()
                .collect();
            for artifact in diff {
                if let Option::Some(ref_count) = self.map.get(&artifact) {
                    rtn.push(Item::new(ref_count.item.clone(), self.tx.clone()));
                }
            }
        }
        let file_access = self.file_access.clone();
        let parser = self.parser.clone();
        let bundle_cache = self.bundle_cache.clone();
        let cache_tx = self.tx.clone();

        tokio::spawn(async move {
            let mut futures = vec![];
            for artifact in fetch_artifacts.clone() {
                futures.push(Self::cache_artifact(
                    artifact,
                    &file_access,
                    &bundle_cache,
                    &parser,
                    &cache_tx,
                ));
            }
            let results = futures::future::join_all(futures).await;
            for result in results {
                if result.is_err() {
                    tx.send(Err(result.err().unwrap()));
                    return;
                }
                if let Ok(cached) = result {
                    rtn.push(cached);
                }
            }

            let mut set = HashSet::new();
            let mut dependencies = HashSet::new();
            for cached in &rtn {
                set.insert(cached.artifact());
                for depend in cached.references() {
                    dependencies.insert(depend);
                }
            }
            let diff = dependencies.difference(&set);
            let diff: Vec<ArtifactRef> = diff.into_iter().cloned().collect();

            if !diff.is_empty() {
                let (tx2, rx2) = oneshot::channel();
                cache_tx
                    .send(RootCacheCall::Cache {
                        artifacts: diff,
                        tx: tx2,
                    })
                    .await;
                rtn.append(&mut match rx2.await {
                    Ok(result) => match result {
                        Ok(cached) => cached,
                        Err(err) => {
                            tx.send(Err(err.into()));
                            return;
                        }
                    },
                    Err(err) => {
                        tx.send(Err(err.into()));
                        return;
                    }
                });
            }

            tx.send(Ok(rtn));
        });
    }

    async fn cache_artifact(
        artifact: ArtifactAddress,
        file_access: &FileAccess,
        bundle_cache: &ArtifactBundleCache,
        parser: &Arc<dyn Parser<C>>,
        cache_tx: &mpsc::Sender<RootCacheCall<C>>,
    ) -> Result<Item<C>, Error> {
        let bundle = artifact.parent();
        bundle_cache.download(bundle.clone().into()).await?;
        let file_access =
            ArtifactBundleCache::with_bundle_files_path(file_access.clone(), bundle.clone())
                .await?;
        let data = file_access.read(&artifact.path()?).await?;
        let item = parser.parse(artifact, data)?;
        Ok(Item::new(Arc::new(item), cache_tx.clone()))
    }
}

#[async_trait]
impl<C: Cacheable> AsyncProcessor<RootCacheCall> for RootCacheProc<C> {
    async fn process(&mut self, call: RootCacheCall) {
        match call {
            RootCacheCall::Cache { artifacts, tx } => {
                self.cache(artifacts, tx).await;
            }
            RootCacheCall::Increment { artifact, item } => {
                let count = self.map.get_mut(&artifact);
                if let Option::Some(count) = count {
                    count.count = count.count + 1;
                } else {
                    self.map.insert(
                        artifact,
                        RefCount {
                            item: item,
                            count: 1,
                        },
                    );
                }
            }
            RootCacheCall::Decrement(artifact) => {
                let count = self.map.get_mut(&artifact);
                if let Option::Some(count) = count {
                    count.count = count.count - 1;
                    if count.count <= 0 {
                        self.map.remove(&artifact);
                    }
                }
            }
        }
    }
}

#[derive(Clone)]
pub enum ArtifactBundleSrc {
    STARLANE_API(StarlaneApi),
    MOCK(MockArtifactBundleSrc),
}

impl ArtifactBundleSrc {
    pub async fn get_resource_state(
        &self,
        identifier: ResourceIdentifier,
    ) -> Result<Option<Arc<Vec<u8>>>, Fail> {
        match self {
            ArtifactBundleSrc::STARLANE_API(api) => api.get_resource_state(identifier),
            ArtifactBundleSrc::MOCK(mock) => mock.get_resource_state(identifier).await,
        }
    }

    pub async fn fetch_resource_record(
        &self,
        identifier: ResourceIdentifier,
    ) -> Result<ResourceRecord, Fail> {
        match self {
            ArtifactBundleSrc::STARLANE_API(api) => api.fetch_resource_record(identifier).await,
            ArtifactBundleSrc::MOCK(mock) => mock.fetch_resource_record(identifier).await,
        }
    }
}

impl From<StarlaneApi> for ArtifactBundleSrc {
    fn from(api: StarlaneApi) -> Self {
        ArtifactBundleSrc::STARLANE_API(api)
    }
}

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
                space: SpaceKey::HyperSpace,
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
                    host: StarKey::central(),
                    gathering: None,
                },
            },
        })
    }
}

impl MockArtifactBundleSrc {
    pub async fn get_resource_state(
        &self,
        identifier: ResourceIdentifier,
    ) -> Result<Option<Arc<Vec<u8>>>, Fail> {
        let mut file = fs::File::open("test-data/localhost-config/artifact-bundle.zip").await?;
        let mut data = vec![];
        file.read_to_end(&mut data).await?;
        Ok(Option::Some(Arc::new(data)))
    }

    pub async fn fetch_resource_record(
        &self,
        identifier: ResourceIdentifier,
    ) -> Result<ResourceRecord, Fail> {
        Ok(self.resource.clone())
    }
}

impl<P> LogInfo for RootItemCache<P>
where
    P: Cacheable,
{
    fn log_identifier(&self) -> String {
        "?".to_string()
    }

    fn log_kind(&self) -> String {
        "?".to_string()
    }

    fn log_object(&self) -> String {
        "RootCache".to_string()
    }
}

#[derive(Clone)]
enum Audit {
    Download(ArtifactBundleAddress),
}

#[derive(Clone)]
struct AuditLogger {
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

struct AuditLogCollector {
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

    pub fn run(mut self) {
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

#[cfg(test)]
mod test {
    use crate::artifact::ArtifactAddress;
    use crate::artifact::ArtifactBundleAddress;
    use crate::cache::{ArtifactBundleCache, ArtifactBundleSrc, ProtoCacheFactory, MockArtifactBundleSrc};
    use crate::error::Error;
    use crate::file_access::FileAccess;
    use std::fs;
    use std::str::FromStr;
    use tokio::runtime::Runtime;
    use tokio::time::{sleep, Duration};

    #[test]
    pub fn some_test() -> Result<(), Error> {
        let data_dir = "tmp/data";
        let cache_dir = "tmp/cache";
        fs::remove_dir_all(data_dir).unwrap_or_default();
        fs::remove_dir_all(cache_dir).unwrap_or_default();
        std::env::set_var("STARLANE_DATA", data_dir);
        std::env::set_var("STARLANE_CACHE", cache_dir);

        let mut builder = tokio::runtime::Builder::new_current_thread();
        let rt = builder.enable_time().enable_io().enable_all().build()?;

        rt.block_on(async {
            async_caches().await;
        });

        Ok(())
    }

    pub async fn async_caches() {
        let caches = ProtoCacheFactory::new(
            MockArtifactBundleSrc::new().unwrap().into(),
            FileAccess::new("tmp/cache".to_string()).unwrap(),
        )
        .unwrap();
        let cache = caches.domain_configs.create();
        let artifact =
            ArtifactAddress::from_str("hyperspace:default:whiz:1.0.0:/routes.txt").unwrap();
        cache.wait_for_cache(artifact.clone()).await.unwrap();
        let cache = cache.into_cache().await.unwrap();
        let config = cache.get(&artifact).unwrap();
    }

    pub async fn async_bundle_test() -> Result<(), Error> {
        let bundle_cache = ArtifactBundleCache::new(
            MockArtifactBundleSrc::new()?.into(),
            FileAccess::new("tmp/cache".to_string())?,
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
