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
use crate::resource::config::Parser;
use crate::message::Fail;
use std::convert::TryInto;

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
        let bundle: ResourceAddress = bundle.into();
        let file_access = self.file_access.with_path(bundle.clone().to_parts_string()).await?;
        file_access.read( &Path::new("/.ready")?).await?;
        Ok(())
    }

    async fn download_and_extract(api: StarlaneApi, mut file_access: FileAccess,  bundle: ArtifactBundleResourceAddress ) -> Result<(),Error> {
        let bundle: ResourceAddress = bundle.into();
        let bundle: ResourceIdentifier = bundle.into();
        let record = api.fetch_resource_record(bundle.clone()).await?;

        let stream = api.get_resource_state(bundle).await?.ok_or("expected bundle to have state")?;

        let mut file_access = file_access.with_path(record.stub.address.clone().to_parts_string()).await?;
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

pub struct ArtifactBundleCache {
    tx: tokio::sync::mpsc::Sender<ArtifactBundleCacheCommand>
}

impl ArtifactBundleCache {
    pub fn new( api: StarlaneApi, file_access: FileAccess ) -> Self {
        let tx = ArtifactBundleCacheRunner::new(api, file_access );
        ArtifactBundleCache{
            tx: tx
        }
    }

    pub async fn download( &self, bundle: ArtifactBundleIdentifier ) -> Result<(),Error>{
        let (tx,rx) = oneshot::channel();
        self.tx.send( ArtifactBundleCacheCommand::Cache {bundle, tx}).await;
        rx.await?
    }
}

pub struct Caches {

}

pub struct Cached<J> {
    address: ArtifactResourceAddress,
    config: Arc<J>,
    claim: oneshot::Sender<ArtifactResourceAddress>,
    cache: Arc<Cache<J>>
}

pub struct Cache<J>{
    pub parser: Parser<J>
}