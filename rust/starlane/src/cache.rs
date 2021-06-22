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
use crate::resource::config::{Parser};
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

pub trait Cacheable: Send+Sync+'static{
    fn artifact(&self)-> ArtifactRef;
    fn references(&self) ->Vec<ArtifactRef>;
}

pub struct ProtoCacheFactory{

}

impl ProtoCacheFactory {
    pub(crate) fn new(src: ArtifactBundleSrc, file_access: FileAccess) -> Result<ProtoCacheFactory,Error> {
        todo!()
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

struct ArtifactBundleCacheRunner {
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
            ArtifactBundleCache::with_bundle_path(self.file_access.clone(), bundle.clone())?;
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
            ArtifactBundleCache::with_bundle_path(file_access, record.stub.address.try_into()?)?;
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

    pub fn with_bundle_files_path(
        file_access: FileAccess,
        address: ArtifactBundleAddress,
    ) -> Result<FileAccess, Error> {
        let address: ResourceAddress = address.into();
        Ok(file_access.with_path(format!("bundles/{}/files", address.to_parts_string()))?)
    }

    pub fn with_bundle_path(
        file_access: FileAccess,
        address: ArtifactBundleAddress,
    ) -> Result<FileAccess, Error> {
        let address: ResourceAddress = address.into();
        Ok(file_access.with_path(format!("bundles/{}", address.to_parts_string()))?)
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

pub struct RefCount<C: Cacheable>{
    pub count: usize,
    pub reference: Arc<C>
}

impl <C:Cacheable> RefCount<C> {
    pub fn new(reference: Arc<C>)->Self{
        RefCount{
            count: 0,
            reference: reference
        }
    }

    pub fn inc(&mut self)
    {
        self.count = self.count + 1;
    }

    pub fn dec(&mut self)
    {
        self.count = self.count - 1;
    }
}

pub struct RootItemCache<C: Cacheable> {
   tx: mpsc::Sender<RootItemCacheCall<C>>
}

impl <C:Cacheable> RootItemCache<C> {

    pub fn new(bundle_cache: ArtifactBundleCache, parser: Arc<dyn Parser<C>>)->Self{
        let (tx,rx) = mpsc::channel(256);

        AsyncRunner::new( Box::new( RootItemCacheProc::new(bundle_cache, parser,tx.clone() )),tx.clone(),rx );

        Self{
            tx: tx
        }
    }

    pub async fn cache( &self, artifact: ArtifactRef ) -> Result<Item<C>,Error>  {
        let (tx,rx)= oneshot::channel();
        self.tx.send( RootItemCacheCall::Cache {artifact,tx}).await?;
        rx.await?
    }

}

impl <C:Cacheable> Call for RootItemCacheCall<C>{

}


pub struct Item<C:Cacheable> {
    item: Arc<C>,
    ref_tx: mpsc::Sender<RootItemCacheCall<C>>
}

impl <C:Cacheable> Item<C> {
    fn new( item: Arc<C>, ref_tx: mpsc::Sender<RootItemCacheCall<C>> ) -> Self {
        let ref_tx_cp = ref_tx.clone();
        let item_cp= item.clone();
        tokio::spawn( async move {
            ref_tx_cp.send( RootItemCacheCall::Increment{artifact:item_cp.artifact(),item: item_cp}).await;
        } );
        Item {
            item: item,
            ref_tx: ref_tx
        }
    }
}

impl <C:Cacheable> Deref for Item<C>{
    type Target = Arc<C>;

    fn deref(&self) -> &Self::Target {
        &self.item
    }
}

impl <C:Cacheable> Clone for Item<C> {
    fn clone(&self) -> Self {

        Item::new( self.item.clone(), self.ref_tx.clone() )
    }
}

pub enum RootItemCacheCall<C:Cacheable>{
    Cache{artifact: ArtifactRef, tx: oneshot::Sender<Result<Item<C>,Error>>},
    Increment{artifact: ArtifactRef, item: Arc<C> },
    Decrement(ArtifactRef),
    Signal{artifact:ArtifactRef, result: Result<Item<C>,Error>}
}

struct RootItemCacheProc<C:Cacheable>{
    bundle_cache: ArtifactBundleCache,
    map: HashMap<ArtifactRef,RefCount<C>>,
    signal_map: HashMap<ArtifactRef,Vec<oneshot::Sender<Result<Item<C>,Error>>>>,
    parser: Arc<dyn Parser<C>>,
    proc_tx: mpsc::Sender<RootItemCacheCall<C>>
}

impl <C:Cacheable> RootItemCacheProc<C>{
    pub fn new(bundle_cache: ArtifactBundleCache, parser: Arc<dyn Parser<C>>, proc_tx: mpsc::Sender<RootItemCacheCall<C>> )-> Self {
        RootItemCacheProc{
            bundle_cache: bundle_cache,
            map: HashMap::new(),
            parser: parser,
            proc_tx: proc_tx,
            signal_map: HashMap::new(),
        }
    }
}

#[async_trait]
impl <C:Cacheable> AsyncProcessor<RootItemCacheCall<C>> for RootItemCacheProc<C>{
    async fn process(&mut self, call: RootItemCacheCall<C>) {
        match call {
            RootItemCacheCall::Increment{ artifact, item } => {
                let ref_count = if self.map.contains_key(&artifact ) {
                    self.map.get_mut(&artifact).unwrap()
                } else {
                    let ref_count = RefCount::new(item );
                    self.map.insert( artifact.clone(), ref_count );
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
                        if( ref_count.count <= 0 ){
                            self.map.remove(&artifact );
                        }
                    }
                }
            }
            RootItemCacheCall::Cache { artifact, tx } => {
                if self.map.contains_key(&artifact){
                    let item = self.map.get(&artifact).unwrap().reference.clone();
                    tx.send( Ok(Item::new(item, self.proc_tx.clone() )) );
                } else {
                    if self.signal_map.contains_key(&artifact) {
                        self.signal_map.get_mut(&artifact).unwrap().push(tx);
                    } else {
                        self.signal_map.insert( artifact.clone(), vec![tx]);
                    }

                    self.cache( artifact).await;
                }
            }
            RootItemCacheCall::Signal { artifact, result } => {
                if let Option::Some(txs) = self.signal_map.remove(&artifact) {
                    for tx in txs {
                        tx.send(result.clone());
                    }
                }
            }
        }
    }



}

impl <C:Cacheable> RootItemCacheProc<C> {

    async fn cache( &self, artifact: ArtifactRef )  {
        let parser = self.parser.clone();
        let bundle_cache = self.bundle_cache.clone();
        let proc_tx = self.proc_tx.clone();

        tokio::spawn( async move {
            match Self::cache_artifact(artifact.clone(), parser.clone(), bundle_cache.clone()).await
            {
                Ok(item) => {
                    proc_tx.send(RootItemCacheCall::Signal{ artifact, result: Ok(Item::new(item, proc_tx.clone())) } ).await;
                }
                Err(err) => {
                    proc_tx.send(RootItemCacheCall::Signal{ artifact, result: Err(err.into()) } ).await;
                }
            }
        });
    }

    async fn cache_artifact<X:Cacheable>( artifact: ArtifactRef, parser: Arc<dyn Parser<X>>, bundle_cache: ArtifactBundleCache ) -> Result<Arc<X>,Error>{
        bundle_cache.download(artifact.artifact.parent().into() ).await?;
        let file_access = ArtifactBundleCache::with_bundle_files_path(bundle_cache.file_access(), artifact.artifact.parent() )?;
        let data = file_access.read(&artifact.artifact.path()?).await?;
        parser.parse( artifact, data )
    }

}


pub struct RootCaches{
    bundle_cache: ArtifactBundleCache,
    domain_configs: RootItemCache<DomainConfig>
}

impl RootCaches{
    pub fn new(bundle_cache: ArtifactBundleCache)->Self {
        Self{
            bundle_cache: bundle_cache.clone(),
            domain_configs: RootItemCache::new(bundle_cache, Arc::new( DomainConfigParser::new() ))
        }
    }
}


#[derive(Clone)]
pub enum Audit {
    Download(ArtifactBundleAddress),
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
    use crate::artifact::{ArtifactAddress, ArtifactRef, ArtifactKind};
    use crate::artifact::ArtifactBundleAddress;
    use crate::cache::{ArtifactBundleCache, ArtifactBundleSrc, ProtoCacheFactory, MockArtifactBundleSrc, AuditLogger, RootItemCache, RootCaches};
    use crate::error::Error;
    use crate::file_access::FileAccess;
    use std::fs;
    use std::str::FromStr;
    use tokio::runtime::Runtime;
    use tokio::time::{sleep, Duration};

    fn reset()
    {
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
        });

        Ok(())
    }

    pub async fn root_item_cache_test() -> Result<(), Error> {
        let bundle_cache = ArtifactBundleCache::new(MockArtifactBundleSrc::new()?.into(), FileAccess::new("tmp/cache".to_string())?, AuditLogger::new() )?;
        let artifact = ArtifactAddress::from_str("hyperspace:default:whiz:1.0.0:/routes.txt")?;
        let artifact = ArtifactRef{
            artifact: artifact,
            kind: ArtifactKind::DomainConfig
        };

        let root_caches = RootCaches::new(bundle_cache);

          let rtn = root_caches.domain_configs.cache(artifact).await;
          assert!(rtn.is_ok());

//        tokio::time::sleep( Duration::from_secs(5)).await;


        Ok(())
    }

    pub async fn async_bundle_test() -> Result<(), Error> {
        let bundle_cache = ArtifactBundleCache::new(MockArtifactBundleSrc::new()?.into(), FileAccess::new("tmp/cache".to_string())?, AuditLogger::new() )?;
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
