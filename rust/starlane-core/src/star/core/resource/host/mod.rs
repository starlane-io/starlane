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
use crate::star::core::resource::host::app::AppHost;
use crate::star::core::resource::host::mechtron::MechtronHost;
use std::sync::Arc;

pub mod artifact;
mod default;
pub mod file_store;
pub mod kube;
mod space;
mod mechtron;
mod app;

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
    hosts: HashMap<ResourceType,Arc<dyn Host>>
}

impl HostComponent {
    pub fn new(skel: StarSkel) -> mpsc::Sender<HostCall> {
        let (tx, rx) = mpsc::channel(1024);
        AsyncRunner::new(Box::new(Self { skel, hosts: HashMap::new() }), tx.clone(), rx);
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
    async fn host(&mut self, rt: ResourceType) -> Arc<dyn Host> {

        if self.hosts.contains_key(&rt) {
            return self.hosts.get(&rt).cloned().expect("expected reference to host");
        }

        let host: Arc<dyn Host> = match rt {
            ResourceType::Space => Arc::new(SpaceHost::new(self.skel.clone()).await),
            ResourceType::SubSpace => Arc::new(SpaceHost::new(self.skel.clone()).await),
            ResourceType::ArtifactBundleVersions => Arc::new(StatelessHost::new(self.skel.clone()).await),
            ResourceType::ArtifactBundle=> Arc::new(ArtifactBundleHost::new(self.skel.clone()).await),
            ResourceType::Domain => Arc::new(StatelessHost::new(self.skel.clone()).await),
            ResourceType::App=> Arc::new(AppHost::new(self.skel.clone()).await),
            ResourceType::Mechtron => Arc::new(MechtronHost::new(self.skel.clone()).await),

            t => unimplemented!("no HOST implementation for type {}", t.to_string()),
        };

        self.hosts.insert( rt, host.clone() );
        host
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
