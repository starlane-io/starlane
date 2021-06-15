use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::thread;

use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::runtime::Runtime;
use tokio::sync::{broadcast, oneshot, mpsc};
use tokio::sync::oneshot::Receiver;

use crate::artifact::{ArtifactBundleId, ArtifactBundleIdentifier, ArtifactBundleResourceAddress, ArtifactIdentifier, ArtifactKey, Artifact};
use crate::error::Error;
use crate::file_access::FileAccess;
use crate::keys::ResourceId;
use crate::resource::{Path, ResourceAddress, ResourceIdentifier, ResourceStub, ResourceRecord};
use crate::starlane::api::StarlaneApi;
use crate::resource::config::{Parser, FromArtifact};
use crate::message::Fail;
use std::convert::TryInto;
use crate::logger::{elog, LogInfo, StaticLogInfo};
use crate::util::{AsyncHashMap, AsyncProcessor, AsyncRunner, Call};
use std::ops::Deref;
use tokio::sync::oneshot::error::RecvError;
use std::future::Future;
use std::iter::FromIterator;
use std::collections::hash_set::Difference;
use crate::resource::domain::{DomainConfig, DomainConfigParser};

pub type Data = Arc<Vec<u8>>;
pub type ZipFile=Path;

pub enum ArtifactBundleCacheCommand {
    Cache{ bundle: ArtifactBundleIdentifier, tx: oneshot::Sender<Result<(),Error>> },
    Result { bundle: ArtifactBundleResourceAddress, result: Result<(),Error> }
}

pub struct ArtifactBundleCacheRunner {
    tx: tokio::sync::mpsc::Sender<ArtifactBundleCacheCommand>,
    rx: tokio::sync::mpsc::Receiver<ArtifactBundleCacheCommand>,
    api: StarlaneApi,
    file_access: FileAccess,
    notify: HashMap<ArtifactBundleResourceAddress,Vec<oneshot::Sender<Result<(),Error>>>>
}

impl ArtifactBundleCacheRunner {
    pub fn new(api: StarlaneApi, file_access: FileAccess) -> tokio::sync::mpsc::Sender<ArtifactBundleCacheCommand> {
        let (tx,rx) = tokio::sync::mpsc::channel(1024);
        let runner = ArtifactBundleCacheRunner {
            file_access: file_access,
            api: api,
            rx: rx,
            tx: tx.clone(),
            notify: HashMap::new()
        };
        thread::spawn( move || {
            let mut builder = tokio::runtime::Builder::new_current_thread();
            let rt = builder.build().expect("<ArtifactBundleCacheRunner> FATAL: could not get tokio runtime");
            rt.block_on(async move {
                runner.run().await;
            });
        });
        tx
    }

    async fn run(mut self) {
        while let Option::Some(command) = self.rx.recv().await {
            match command {
                ArtifactBundleCacheCommand::Cache{ bundle,tx} => {
                    let bundle: ResourceIdentifier = bundle.into();
                    let record = match self.api.fetch_resource_record(bundle.clone()).await{
                        Ok(record) => record,
                        Err(err) => {
                            tx.send( Err(err.into()));
                            continue;
                        }
                    };
                    let bundle:ArtifactBundleResourceAddress = match record.stub.address.try_into() {
                        Ok(ok) => ok,
                        Err(err) => {
                            tx.send( Err(err.into()));
                            continue;
                        }
                    };

                    if self.has(bundle.clone()).await.is_ok() {
                        tx.send(Ok(()) );
                    } else {
                        let first = if !self.notify.contains_key(&bundle) {
                            self.notify.insert(bundle.clone(), vec![] );
                            true
                        } else {
                            false
                        };

                        let notifiers = self.notify.get_mut(&bundle).unwrap();
                        notifiers.push(tx);

                        let api = self.api.clone();
                        let file_access = self.file_access.clone();
                        let tx = self.tx.clone();
                        if first {
                            tokio::spawn(async move {
                                let result = Self::download_and_extract(api, file_access, bundle.clone()).await;
                                tx.send(ArtifactBundleCacheCommand::Result { bundle: bundle.clone(), result: result }).await;
                            });
                        }
                    }
                }
                ArtifactBundleCacheCommand::Result{bundle,result} => {
                    let notifiers = self.notify.remove(&bundle );
                    if let Option::Some(mut notifiers) = notifiers {
                        for notifier in notifiers.drain(..){
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

    async fn has( &self, bundle: ArtifactBundleResourceAddress ) -> Result<(),Error>{
        let file_access = ArtifactBundleCache::with_bundle_path(self.file_access.clone(), bundle.clone()).await?;
        file_access.read( &Path::new("/.ready")?).await?;
        Ok(())
    }

    async fn download_and_extract(api: StarlaneApi, mut file_access: FileAccess,  bundle: ArtifactBundleResourceAddress ) -> Result<(),Error> {
        let bundle: ResourceAddress = bundle.into();
        let bundle: ResourceIdentifier = bundle.into();
        let record = api.fetch_resource_record(bundle.clone()).await?;

        let stream = api.get_resource_state(bundle).await?.ok_or("expected bundle to have state")?;

        let mut file_access = ArtifactBundleCache::with_bundle_path(file_access,record.stub.address.try_into()?).await?;
        let bundle_zip = Path::new("/bundle.zip")?;
        let key_file = Path::new("/key.ser")?;
        file_access.write(&key_file, Arc::new(record.stub.key.to_string().as_bytes().to_vec()));
        file_access.write(&bundle_zip, stream).await?;

        file_access.unzip("bundle.zip".to_string(), "files".to_string()).await?;

        let ready_file= Path::new(".ready")?;
        file_access.write(&ready_file, Arc::new("READY".to_string().as_bytes().to_vec()));

        Ok(())
    }


}

#[derive(Clone)]
pub struct ArtifactBundleCache {
    file_access: FileAccess,
    tx: tokio::sync::mpsc::Sender<ArtifactBundleCacheCommand>
}

impl ArtifactBundleCache {
    pub async fn new( api: StarlaneApi, file_access: FileAccess ) -> Result<Self,Error> {
        let file_access = file_access.with_path("bundles".to_string() ).await?;
        let tx = ArtifactBundleCacheRunner::new(api, file_access.clone() );
        Ok(ArtifactBundleCache{
            file_access: file_access,
            tx: tx
        })
    }

    pub async fn download( &self, bundle: ArtifactBundleIdentifier ) -> Result<(),Error>{
        let (tx,rx) = oneshot::channel();
        self.tx.send( ArtifactBundleCacheCommand::Cache {bundle, tx}).await;
        rx.await?
    }

    pub fn file_access(&self)->FileAccess {
        self.file_access.clone()
    }

    pub async fn with_bundle_path(file_access:FileAccess, address: ArtifactBundleResourceAddress ) -> Result<FileAccess,Error> {
        let address: ResourceAddress = address.into();
        Ok(file_access.with_path(address.to_parts_string() ).await?)
    }
}

pub struct Cache<C: Cacheable> {
    root: Arc<RootCache<C>>,
    map: HashMap<Artifact,Cached<C>>
}

impl <C:Cacheable> Cache<C> {

    fn new( root: Arc<RootCache<C>> ) -> Self {
        Self{
            root: root,
            map: HashMap::new()
        }
    }

    pub async fn cache(&self, artifacts: Vec<Artifact> ) -> oneshot::Receiver<Result<(),Error>>{
        let (tx,rx) = oneshot::channel();

        let parent_rx = self.root.cache(artifacts ).await;

        tokio::spawn( async move {
            match Self::flatten::<C>(parent_rx).await {
                Ok(cached) => {
                    tx.send(Ok(()));
                }
                Err(err) => {
                    tx.send(Err(err.into()));
                }
            }
        });

        rx
    }

    async fn flatten<X: Cacheable>( parent_rx: oneshot::Receiver<Result<Vec<Cached<X>>,Error>> ) -> Result<Vec<Cached<X>>,Error> {
        Ok(parent_rx.await??)
    }

    pub fn get(&self, artifact: &Artifact) -> Result<Cached<C>,Error>{
        let cached = self.map.get(artifact);
        if let Some(cached) = cached {
            Ok(cached.clone())
        }
        else{
            Err(format!("must call cache.cache('{}') for this artifact and wait for cache callback before get()",artifact.to_string()).into())
        }
    }

}


pub struct CacheFactory<C: Cacheable>{
    root_cache: Arc<RootCache<C>>
}

impl <C:Cacheable> CacheFactory<C> {
    fn new(root_cache: Arc<RootCache<C>>) -> Self {
       Self{
           root_cache: root_cache
       }
    }

    pub fn create(&self) -> Cache<C> {
        Cache::new(self.root_cache.clone())
    }
}

pub struct CacheFactories {
    pub domain_configs: CacheFactory<DomainConfig>
}

impl CacheFactories {
    async fn new(api: StarlaneApi, file_access: FileAccess) -> Result<CacheFactories,Error>{

        let domain_configs = {
            let parser = Arc::new(DomainConfigParser::new());
            let configs = Arc::new(RootCache::new(api, file_access, parser).await?);
            CacheFactory::new(configs)
        };

        Ok(Self{
           domain_configs
        })
    }
}

pub trait Cacheable: FromArtifact+Send+Sync+'static {}


struct RootCache<C> where C: Cacheable {
   tx: mpsc::Sender<CacheCall<C>>
}

impl <C:Cacheable> RootCache<C> {
    async fn new(api: StarlaneApi, file_access: FileAccess, parser: Arc<dyn Parser<C>>)->Result<Self,Error>{
        Ok(RootCache {
            tx: RootCacheProc::new(api, file_access, parser).await?
        })
    }

    async fn cache(&self, artifacts: Vec<Artifact> ) -> oneshot::Receiver<Result<Vec<Cached<C>>,Error>>{
        let (tx,rx) = oneshot::channel();
        self.tx.send( CacheCall::Cache {artifacts, tx }).await;
        rx
    }

}

pub enum CacheCall<C:Cacheable> {
   Cache{artifacts: Vec<Artifact>, tx: oneshot::Sender<Result<Vec<Cached<C>>,Error>>},
   Increment{artifact: Artifact, item: Arc<C>},
   Decrement(Artifact)
}

impl <C:Cacheable> Call for CacheCall<C> {

}

pub struct Cached<C:Cacheable> {
    item: Arc<C>,
    deref_tx: mpsc::Sender<CacheCall<C>>
}

impl <C:Cacheable> Cached<C>{
    pub fn new(item:Arc<C>, deref_tx:mpsc::Sender<CacheCall<C>>) -> Self{

        let item_cp = item.clone();
        let deref_tx_cp= deref_tx.clone();
        tokio::spawn( async move {
            deref_tx_cp.send(CacheCall::Increment{ artifact: item_cp.artifact(), item: item_cp } ).await;
        });

        Cached{
            item: item,
            deref_tx: deref_tx
        }
    }
}

impl <C:Cacheable> Clone for Cached<C>{

    fn clone(&self) -> Self {
        Self::new( self.item.clone(), self.deref_tx.clone() )
    }
}

impl <C:Cacheable> Deref for Cached<C>{
    type Target = Arc<C>;

    fn deref(&self) -> &Self::Target {
        &self.item
    }
}

impl <C:Cacheable> Drop for Cached<C> {
    fn drop(&mut self) {
        self.deref_tx.send(CacheCall::Decrement(self.item.artifact()));
    }
}

struct RefCount<C> {
    pub count: usize,
    pub item: Arc<C>
}

struct RootCacheProc<C:Cacheable>{
    pub tx: mpsc::Sender<CacheCall<C>>,
    pub parser: Arc<dyn Parser<C>>,
    pub bundle_cache: ArtifactBundleCache,
    pub map: HashMap<Artifact,RefCount<C>>,
    pub file_access: FileAccess
}

impl <C:Cacheable> RootCacheProc<C> {
    pub async fn new(api: StarlaneApi, file_access: FileAccess, parser: Arc<dyn Parser<C>>) -> Result<mpsc::Sender<CacheCall<C>>,Error> {

        let (tx,rx) = mpsc::channel(16);
        Ok(AsyncRunner::new(
            Box::new(
                      RootCacheProc {
                          tx: tx.clone(),
                          parser: parser,
                          file_access: file_access.clone(),
                          bundle_cache: ArtifactBundleCache::new(api,file_access).await?,
                           map: HashMap::new()
                      }),tx,rx))
    }

    async fn cache(&mut self, artifacts: Vec<Artifact>, tx: oneshot::Sender<Result<Vec<Cached<C>>,Error>> )
    {
        let mut rtn = vec![];
        let mut fetch_artifacts = artifacts.clone();
        // these are the ones we don't have in our cache yet
        fetch_artifacts.retain( |artifact| !self.map.contains_key(artifact));

        {
            let artifacts:HashSet<Artifact> = HashSet::from_iter(artifacts);
            let fetch_artifacts:HashSet<Artifact> = HashSet::from_iter(fetch_artifacts.clone());
            let diff:Vec<Artifact> = artifacts.difference(&fetch_artifacts).into_iter().cloned().collect();
            for artifact in diff {
                if let Option::Some(ref_count)  = self.map.get(&artifact)
                {
                    rtn.push(Cached::new(ref_count.item.clone(), self.tx.clone() ) );
                }
            }
        }
        let file_access = self.file_access.clone();
        let parser = self.parser.clone();
        let bundle_cache = self.bundle_cache.clone();
        let cache_tx = self.tx.clone();

        tokio::spawn( async move {
           let mut futures = vec![];
           for artifact in fetch_artifacts.clone() {
                futures.push(Self::cache_artifact( artifact, &file_access, &bundle_cache, &parser, &cache_tx ));
           }
           let results = futures::future::join_all(futures).await;
           for result in results {
                if result.is_err() {
                    tx.send( Err(result.err().unwrap()) );
                    return;
                }
                if let Ok(cached) = result {
                    rtn.push(cached);
                }
            }

            let mut set = HashSet::new();
            let mut dependencies = HashSet::new();
            for cached in &rtn {
                set.insert(cached.artifact() );
                for depend in cached.dependencies() {
                    dependencies.insert(depend);
                }
            }
            let diff = dependencies.difference(&set);
            let diff:Vec<Artifact> = diff.into_iter().cloned().collect();

            if !diff.is_empty() {
                let (tx2,rx2) = oneshot::channel();
                cache_tx.send( CacheCall::Cache{ artifacts: diff, tx: tx2 }).await;
                rtn.append( & mut match rx2.await {
                    Ok(result) => {
                        match result {
                            Ok(cached) => {
                                cached
                            }
                            Err(err) => {
                                tx.send(Err(err.into()));
                                return;
                            }
                        }
                    }
                    Err(err) => {
                        tx.send(Err(err.into()));
                        return;
                    }
                });
            }

            tx.send(Ok(rtn));
        });
    }

    async fn cache_artifact(artifact: Artifact, file_access: &FileAccess, bundle_cache: &ArtifactBundleCache, parser: &Arc<dyn Parser<C>>, cache_tx: &mpsc::Sender<CacheCall<C>>) -> Result<Cached<C>,Error>{
        let bundle = artifact.parent();
        bundle_cache.download(bundle.clone().into() ).await?;
        let file_access = ArtifactBundleCache::with_bundle_path(file_access.clone(), bundle.clone()).await?;
        let data = file_access.read( &artifact.path()? ).await?;
        let item = parser.parse(artifact, data )?;
        Ok( Cached::new(  Arc::new(item), cache_tx.clone() ) )
    }
}

#[async_trait]
impl <C:Cacheable> AsyncProcessor<CacheCall<C>> for RootCacheProc<C>{
    async fn process(&mut self, call: CacheCall<C>) {
        match call{
            CacheCall::Cache { artifacts, tx } => {

            }
            CacheCall::Increment{artifact,item} => {
                let count = self.map.get_mut(&artifact);
                if let Option::Some(count) = count {
                    count.count = count.count + 1;
                } else {
                    self.map.insert( artifact, RefCount{
                        item: item,
                        count: 1
                    });
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


impl <P> LogInfo for RootCache<P> where P: Cacheable {
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