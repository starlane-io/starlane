use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::thread;

use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::runtime::Runtime;
use tokio::sync::{broadcast, oneshot};
use tokio::sync::oneshot::Receiver;

use crate::artifact::{ArtifactBundleId, ArtifactBundleIdentifier, ArtifactBundleResourceAddress, ArtifactIdentifier, ArtifactKey, ArtifactResourceAddress};
use crate::error::Error;
use crate::file_access::FileAccess;
use crate::keys::ResourceId;
use crate::resource::{Path, ResourceAddress, ResourceIdentifier, ResourceStub, ResourceRecord};
use crate::starlane::api::StarlaneApi;
use crate::resource::config::{Parser, FromArtifact};
use crate::message::Fail;
use std::convert::TryInto;
use crate::logger::{elog, LogInfo, StaticLogInfo};
use crate::util::AsyncHashMap;

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

pub struct Caches {

}

pub struct Cached<J> where J: FromArtifact+Clone+Send+Sync+'static {
    address: ArtifactResourceAddress,
    config: Arc<J>,
    claim: oneshot::Sender<ArtifactResourceAddress>,
    cache: Arc<ParentCache<J>>
}

pub struct ParentCache<J> where J: FromArtifact+Clone+Send+Sync+'static {
    map: AsyncHashMap<ArtifactResourceAddress,Result<Arc<J>,Error>>,
    bundle_cache: ArtifactBundleCache,
    parser: Arc<dyn Parser<J>>,
    file_access: FileAccess
}

impl <J> ParentCache<J> where J: FromArtifact+Clone+Send+Sync+'static{
    pub async fn cache(&self, artifact: ArtifactResourceAddress ) -> oneshot::Receiver<Result<(),Error>>{
       let (tx,rx) = oneshot::channel();

        if let Ok(Option::Some(result)) = self.map.get(artifact.clone() ).await {
            match result {
                Ok(_) => {
                    tx.send(Ok(()));
                }
                Err(err) => {
                    tx.send(Err(err.into()));
                }
            }
        } else {
            let log_info = StaticLogInfo::clone_info(Box::new(self));
            let file_access = self.file_access.clone();
            let bundle_cache = self.bundle_cache.clone();
            let parser = self.parser.clone();
            let map = self.map.clone();
            tokio::spawn(async move {
                let result = Self::cache_final::<J>(bundle_cache, artifact, file_access, parser, map ).await;

                match result {
                    Ok(ok) => {
                        tx.send(Ok(()));
                    }
                    Err(err) => {
                        tx.send(Err(err.into()));
                    }
                }
            });
        }

       rx
    }

    async fn cache_final<X>( bundle_cache: ArtifactBundleCache, artifact: ArtifactResourceAddress, file_access: FileAccess, parser: Arc<dyn Parser<X>>, mut map: AsyncHashMap<ArtifactResourceAddress,Result<Arc<X>,Error>>) -> Result<Arc<X>,Error> where X: FromArtifact+Clone+Send+Sync+'static{
        bundle_cache.download(artifact.parent().into()).await?;
        let bundle = artifact.parent();
        let file_access = ArtifactBundleCache::with_bundle_path(file_access.clone(), bundle.clone()).await?;
        let data = file_access.read( &artifact.path()? ).await?;
        let result = Ok(Arc::new(parser.parse(artifact.clone(), data)?));
        map.put( artifact, result.clone() ).await;

        result
    }

    pub async fn get(&self, artifact: &ArtifactResourceAddress ) -> Result<Arc<J>,Error>{
        Ok(self.map.get(artifact.clone() ).await?.ok_or(format!("ERROR: attempt to get artifact '{}' from cache before calling the cache() method.",artifact.to_string() ) )??)
    }
}




impl <P> LogInfo for ParentCache<P> where P: FromArtifact+Clone+Send+Sync+'static {
    fn log_identifier(&self) -> String {
        "?".to_string()
    }

    fn log_kind(&self) -> String {
        "?".to_string()
    }

    fn log_object(&self) -> String {
        "ParentCache".to_string()
    }
}