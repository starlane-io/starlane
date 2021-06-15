use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::thread;

use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::runtime::{Handle, Runtime};
use tokio::sync::oneshot::Receiver;
use tokio::sync::{broadcast, mpsc, oneshot};

use crate::artifact::{
    Artifact, ArtifactBundleId, ArtifactBundleIdentifier, ArtifactBundleKey, ArtifactIdentifier,
    ArtifactKey, Bundle,
};
use crate::error::Error;
use crate::file_access::FileAccess;
use crate::keys::{ResourceId, ResourceKey, SpaceKey, SubSpaceKey};
use crate::logger::{elog, LogInfo, StaticLogInfo};
use crate::message::Fail;
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

pub type Data = Arc<Vec<u8>>;
pub type ZipFile = Path;

pub struct Caches {
    pub domain_configs: CacheFactory<DomainConfig>,
}

impl Caches {
    pub fn new(src: ArtifactBundleSrc, file_access: FileAccess) -> Result<Caches, Error> {
        let domain_configs = {
            let parser = Arc::new(DomainConfigParser::new());
            let configs = Arc::new(RootCache::new(src, file_access, parser)?);
            CacheFactory::new(configs)
        };

        Ok(Self { domain_configs })
    }
}

pub enum ArtifactBundleCacheCommand {
    Cache {
        bundle: ArtifactBundleIdentifier,
        tx: oneshot::Sender<Result<(), Error>>,
    },
    Result {
        bundle: Bundle,
        result: Result<(), Error>,
    },
}

pub struct ArtifactBundleCacheRunner {
    tx: tokio::sync::mpsc::Sender<ArtifactBundleCacheCommand>,
    rx: tokio::sync::mpsc::Receiver<ArtifactBundleCacheCommand>,
    src: ArtifactBundleSrc,
    file_access: FileAccess,
    notify: HashMap<Bundle, Vec<oneshot::Sender<Result<(), Error>>>>,
}

impl ArtifactBundleCacheRunner {
    pub fn new(
        src: ArtifactBundleSrc,
        file_access: FileAccess,
    ) -> tokio::sync::mpsc::Sender<ArtifactBundleCacheCommand> {
        let (tx, rx) = tokio::sync::mpsc::channel(1024);
        let runner = ArtifactBundleCacheRunner {
            file_access: file_access,
            src: src,
            rx: rx,
            tx: tx.clone(),
            notify: HashMap::new(),
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
        println!("RUNNER RUNNING!");
        while let Option::Some(command) = self.rx.recv().await {
            match command {
                ArtifactBundleCacheCommand::Cache { bundle, tx } => {
                    let bundle: ResourceIdentifier = bundle.into();
println!("getting resource record...");
                    let record = match self.src.fetch_resource_record(bundle.clone()).await {
                        Ok(record) => record,
                        Err(err) => {
                            tx.send(Err(err.into()));
                            continue;
                        }
                    };
                    let bundle: Bundle = match record.stub.address.try_into() {
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
                            tokio::spawn(async move {
                                let result =
                                    Self::download_and_extract(src, file_access, bundle.clone())
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

    async fn has(&self, bundle: Bundle) -> Result<(), Error> {
        let file_access =
            ArtifactBundleCache::with_bundle_path(self.file_access.clone(), bundle.clone()).await?;
        file_access.read(&Path::new("/.ready")?).await?;
        Ok(())
    }

    async fn download_and_extract(
        src: ArtifactBundleSrc,
        mut file_access: FileAccess,
        bundle: Bundle,
    ) -> Result<(), Error> {
        let bundle: ResourceAddress = bundle.into();
        let bundle: ResourceIdentifier = bundle.into();
        let record = src.fetch_resource_record(bundle.clone()).await?;

        let stream = src
            .get_resource_state(bundle)
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
        file_access.write(
            &ready_file,
            Arc::new("READY".to_string().as_bytes().to_vec()),
        ).await?;

        Ok(())
    }
}

#[derive(Clone)]
pub struct ArtifactBundleCache {
    file_access: FileAccess,
    tx: tokio::sync::mpsc::Sender<ArtifactBundleCacheCommand>,
}

impl ArtifactBundleCache {
    pub fn new(src: ArtifactBundleSrc, file_access: FileAccess) -> Result<Self, Error> {
        let file_access = file_access.with_path("bundles".to_string())?;
        let tx = ArtifactBundleCacheRunner::new(src, file_access.clone());
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

    pub async fn with_bundle_path(
        file_access: FileAccess,
        address: Bundle,
    ) -> Result<FileAccess, Error> {
        let address: ResourceAddress = address.into();
        Ok(file_access.with_path(address.to_parts_string())?)
    }
}

#[derive(Clone)]
pub struct Cache<C: Cacheable> {
    root: Arc<RootCache<C>>,
    map: HashMap<Artifact, Cached<C>>,
}

impl<C: Cacheable> Into<ProtoCache<C>> for Cache<C> {
    fn into(self) -> ProtoCache<C> {
        ProtoCache {
            map: AsyncHashMap::from(self.map),
            root: self.root,
        }
    }
}

impl<C: Cacheable> Cache<C> {
    pub fn get(&self, artifact: &Artifact) -> Result<Cached<C>, Error> {
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
pub struct ProtoCache<C: Cacheable> {
    root: Arc<RootCache<C>>,
    map: AsyncHashMap<Artifact, Cached<C>>,
}

impl<C: Cacheable> ProtoCache<C> {
    fn new(root: Arc<RootCache<C>>) -> Self {
        Self {
            root: root,
            map: AsyncHashMap::new(),
        }
    }

    pub async fn wait_for_cache(&self, artifact: Artifact) -> Result<(), Error> {
        self.cache(artifact).await?
    }

    pub async fn wait_for_cache_all(&self, artifacts: Vec<Artifact>) -> Result<(), Error> {
        self.cache_all(artifacts).await?
    }

    pub fn cache(&self, artifact: Artifact) -> oneshot::Receiver<Result<(), Error>> {
        self.cache_all(vec![artifact])
    }

    pub fn cache_all(&self, artifacts: Vec<Artifact>) -> oneshot::Receiver<Result<(), Error>> {
        let (tx, rx) = oneshot::channel();

        let parent_rx = self.root.cache(artifacts);

        let map = self.map.clone();
        tokio::spawn(async move {
            match Self::flatten::<C>(parent_rx).await {
                Ok(cached) => {
                    for c in cached {
                        match map.put(c.artifact(), c).await {
                            Ok(_) => {}
                            Err(error) => {
                                eprintln!("!<Cache> FATAL: could not put Cached<C>");
                            }
                        }
                    }
                    tx.send(Ok(()));
                }
                Err(err) => {
                    tx.send(Err(err.into()));
                }
            }
        });

        rx
    }

    async fn flatten<X: Cacheable>(
        parent_rx: oneshot::Receiver<Result<Vec<Cached<X>>, Error>>,
    ) -> Result<Vec<Cached<X>>, Error> {
        Ok(parent_rx.await??)
    }

    pub fn get(&self, artifact: &Artifact) -> Result<Cached<C>, Error> {
        let handle = Handle::current();
        handle.block_on( async move {
            let cached = self.map.get(artifact.clone()).await?;
            if let Some(cached) = cached {
                Ok(cached.clone())
            }
            else{
                Err(format!("must call cache.cache('{}') for this artifact and wait for cache callback before get()",artifact.to_string()).into())
            }
        })
    }

    pub async fn into_cache(self) -> Result<Cache<C>, Error> {
        Ok(Cache {
            map: self.map.into_map().await?,
            root: self.root,
        })
    }
}

pub struct CacheFactory<C: Cacheable> {
    root_cache: Arc<RootCache<C>>,
}

impl<C: Cacheable> CacheFactory<C> {
    fn new(root_cache: Arc<RootCache<C>>) -> Self {
        Self {
            root_cache: root_cache,
        }
    }

    pub fn create(&self) -> ProtoCache<C> {
        ProtoCache::new(self.root_cache.clone())
    }
}

pub trait Cacheable: FromArtifact + Send + Sync + 'static {}

struct RootCache<C>
where
    C: Cacheable,
{
    tx: mpsc::Sender<CacheCall<C>>,
}

impl<C: Cacheable> RootCache<C> {
    fn new(
        src: ArtifactBundleSrc,
        file_access: FileAccess,
        parser: Arc<dyn Parser<C>>,
    ) -> Result<Self, Error> {
        Ok(RootCache {
            tx: RootCacheProc::new(src, file_access, parser)?,
        })
    }

    fn cache(&self, artifacts: Vec<Artifact>) -> oneshot::Receiver<Result<Vec<Cached<C>>, Error>> {
        let (tx, rx) = oneshot::channel();

        let cache_tx = self.tx.clone();
        tokio::spawn(async move {
            cache_tx.send(CacheCall::Cache { artifacts, tx }).await;
        });

        rx
    }
}

pub enum CacheCall<C: Cacheable> {
    Cache {
        artifacts: Vec<Artifact>,
        tx: oneshot::Sender<Result<Vec<Cached<C>>, Error>>,
    },
    Increment {
        artifact: Artifact,
        item: Arc<C>,
    },
    Decrement(Artifact),
}

impl<C: Cacheable> Call for CacheCall<C> {}

pub struct Cached<C: Cacheable> {
    item: Arc<C>,
    deref_tx: mpsc::Sender<CacheCall<C>>,
}

impl<C: Cacheable> Cached<C> {
    pub fn new(item: Arc<C>, deref_tx: mpsc::Sender<CacheCall<C>>) -> Self {
        let item_cp = item.clone();
        let deref_tx_cp = deref_tx.clone();
        tokio::spawn(async move {
            deref_tx_cp
                .send(CacheCall::Increment {
                    artifact: item_cp.artifact(),
                    item: item_cp,
                })
                .await;
        });

        Cached {
            item: item,
            deref_tx: deref_tx,
        }
    }
}

impl<C: Cacheable> Clone for Cached<C> {
    fn clone(&self) -> Self {
        Self::new(self.item.clone(), self.deref_tx.clone())
    }
}

impl<C: Cacheable> Deref for Cached<C> {
    type Target = Arc<C>;

    fn deref(&self) -> &Self::Target {
        &self.item
    }
}

impl<C: Cacheable> Drop for Cached<C> {
    fn drop(&mut self) {
        self.deref_tx
            .send(CacheCall::Decrement(self.item.artifact()));
    }
}

struct RefCount<C> {
    pub count: usize,
    pub item: Arc<C>,
}

struct RootCacheProc<C: Cacheable> {
    pub tx: mpsc::Sender<CacheCall<C>>,
    pub parser: Arc<dyn Parser<C>>,
    pub bundle_cache: ArtifactBundleCache,
    pub map: HashMap<Artifact, RefCount<C>>,
    pub file_access: FileAccess,
}

impl<C: Cacheable> RootCacheProc<C> {
    pub fn new(
        src: ArtifactBundleSrc,
        file_access: FileAccess,
        parser: Arc<dyn Parser<C>>,
    ) -> Result<mpsc::Sender<CacheCall<C>>, Error> {
        let (tx, rx) = mpsc::channel(16);
        Ok(AsyncRunner::new(
            Box::new(RootCacheProc {
                tx: tx.clone(),
                parser: parser,
                file_access: file_access.clone(),
                bundle_cache: ArtifactBundleCache::new(src, file_access)?,
                map: HashMap::new(),
            }),
            tx,
            rx,
        ))
    }

    async fn cache(
        &mut self,
        artifacts: Vec<Artifact>,
        tx: oneshot::Sender<Result<Vec<Cached<C>>, Error>>,
    ) {
        let mut rtn = vec![];
        let mut fetch_artifacts = artifacts.clone();
        // these are the ones we don't have in our cache yet
        fetch_artifacts.retain(|artifact| !self.map.contains_key(artifact));

        {
            let artifacts: HashSet<Artifact> = HashSet::from_iter(artifacts);
            let fetch_artifacts: HashSet<Artifact> = HashSet::from_iter(fetch_artifacts.clone());
            let diff: Vec<Artifact> = artifacts
                .difference(&fetch_artifacts)
                .into_iter()
                .cloned()
                .collect();
            for artifact in diff {
                if let Option::Some(ref_count) = self.map.get(&artifact) {
                    rtn.push(Cached::new(ref_count.item.clone(), self.tx.clone()));
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
                println!("pushign future...");
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
                for depend in cached.dependencies() {
                    dependencies.insert(depend);
                }
            }
            let diff = dependencies.difference(&set);
            let diff: Vec<Artifact> = diff.into_iter().cloned().collect();

            if !diff.is_empty() {
                let (tx2, rx2) = oneshot::channel();
                cache_tx
                    .send(CacheCall::Cache {
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
        artifact: Artifact,
        file_access: &FileAccess,
        bundle_cache: &ArtifactBundleCache,
        parser: &Arc<dyn Parser<C>>,
        cache_tx: &mpsc::Sender<CacheCall<C>>,
    ) -> Result<Cached<C>, Error> {
        let bundle = artifact.parent();
        bundle_cache.download(bundle.clone().into()).await?;
        let file_access =
            ArtifactBundleCache::with_bundle_path(file_access.clone(), bundle.clone()).await?;
        let data = file_access.read(&artifact.path()?).await?;
        let item = parser.parse(artifact, data)?;
        println!("cache artifact RTN!");
        Ok(Cached::new(Arc::new(item), cache_tx.clone()))
    }
}

#[async_trait]
impl<C: Cacheable> AsyncProcessor<CacheCall<C>> for RootCacheProc<C> {
    async fn process(&mut self, call: CacheCall<C>) {
        match call {
            CacheCall::Cache { artifacts, tx } => {}
            CacheCall::Increment { artifact, item } => {
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
            CacheCall::Decrement(artifact) => {
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
        let handle = Handle::current();

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

impl<P> LogInfo for RootCache<P>
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

#[cfg(test)]
mod test {
    use crate::artifact::Bundle;
    use crate::cache::{ArtifactBundleCache, ArtifactBundleSrc, MockArtifactBundleSrc};
    use crate::error::Error;
    use crate::file_access::FileAccess;
    use std::str::FromStr;
    use tokio::runtime::Runtime;
    use tokio::time::{sleep, Duration};

    #[test]
    pub fn some_test() -> Result<(), Error> {
        let mut builder = tokio::runtime::Builder::new_current_thread();
        let rt = builder.enable_time().enable_io().enable_all().build()?;

        rt.block_on(async {
            async_bundle_test().await.unwrap();
        });

        Ok(())
    }

    pub async fn async_bundle_test() -> Result<(), Error> {
        let bundle_cache = ArtifactBundleCache::new(
            MockArtifactBundleSrc::new()?.into(),
            FileAccess::new("tmp/cache".to_string())?,
        )?;
        let bundle = Bundle::from_str("hyperspace:default:whiz:1.0.0")?;
        println!("GOT HERE..");
        bundle_cache.download(bundle.into()).await?;
        println!("Retrurned from DOWNLOAD..");

        Ok(())
    }
}
