use crate::data::{BinSrc, DataSet};
use crate::error::Error;
use crate::message::Fail;
use crate::resource::{
    AssignResourceStateSrc, Resource, ResourceAssign, ResourceKey, ResourceType,
};
use crate::star::core::resource::host::default::StatelessHost;
use crate::star::core::resource::host::space::SpaceHost;
use crate::star::StarSkel;
use crate::util::{AsyncProcessor, AsyncRunner, Call};
use std::collections::hash_map::RandomState;
use std::collections::HashMap;
use tokio::sync::{mpsc, oneshot};
use crate::star::core::resource::host::artifact::ArtifactBundleHost;

pub mod artifact;
mod default;
pub mod file_store;
pub mod kube;
mod space;

pub enum HostCall {
    Assign {
        assign: ResourceAssign<AssignResourceStateSrc<DataSet<BinSrc>>>,
        tx: oneshot::Sender<Result<Resource, Fail>>,
    },
    Get {
        key: ResourceKey,
        tx: oneshot::Sender<Result<Option<DataSet<BinSrc>>, Fail>>,
    },
    Has {
        key: ResourceKey,
        tx: oneshot::Sender<bool>,
    },
}

impl Call for HostCall {}

pub struct HostComponent {
    skel: StarSkel,
}

impl HostComponent {
    pub fn new(skel: StarSkel) -> mpsc::Sender<HostCall> {
        let (tx, rx) = mpsc::channel(1024);
        AsyncRunner::new(Box::new(Self { skel }), tx.clone(), rx);
        tx
    }
}

#[async_trait]
impl AsyncProcessor<HostCall> for HostComponent {
    async fn process(&mut self, call: HostCall) {
        match call {
            HostCall::Get { key, tx } => {
                let host = self.host(key.resource_type()).await;
                tx.send(host.get(key).await);
            }
            HostCall::Assign { assign, tx } => {
                let host = self.host(assign.stub.key.resource_type()).await;
                match host.assign(assign.clone()).await {
                    Ok(state) => {
                        let resource = Resource::new(
                            assign.stub.key,
                            assign.stub.address,
                            assign.stub.archetype,
                            state,
                        );
                        tx.send(Ok(resource));
                    }
                    Err(err) => {
                        tx.send(Err(err));
                    }
                }
            }
            HostCall::Has { key, tx } => {
                let host = self.host(key.resource_type()).await;
                tx.send(host.has(key).await);
            }
        }
    }
}

impl HostComponent {
    async fn host(&self, rt: ResourceType) -> Box<dyn Host> {
        match rt {
            ResourceType::Space => Box::new(SpaceHost::new(self.skel.clone()).await),
            ResourceType::SubSpace => Box::new(SpaceHost::new(self.skel.clone()).await),
            ResourceType::ArtifactBundleVersions => Box::new(StatelessHost::new(self.skel.clone()).await),
            ResourceType::ArtifactBundle=> Box::new(ArtifactBundleHost::new(self.skel.clone()).await),
            ResourceType::Domain => Box::new(StatelessHost::new(self.skel.clone()).await),

            t => unimplemented!("no HOST implementation for type {}", t.to_string()),
        }
    }
}

#[async_trait]
pub trait Host: Send + Sync {
    async fn assign(
        &self,
        assign: ResourceAssign<AssignResourceStateSrc<DataSet<BinSrc>>>,
    ) -> Result<DataSet<BinSrc>, Fail>;
    async fn has(&self, key: ResourceKey) -> bool;
    async fn get(&self, key: ResourceKey) -> Result<Option<DataSet<BinSrc>>, Fail>;
    async fn delete(&self, key: ResourceKey) -> Result<(), Fail>;
    fn shutdown(&self) {}
}
