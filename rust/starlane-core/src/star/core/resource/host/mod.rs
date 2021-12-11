use std::collections::hash_map::RandomState;
use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{mpsc, oneshot};

use crate::error::Error;
use crate::resource::{ResourceType, AssignResourceStateSrc, ResourceAssign};
use crate::star::core::resource::host::app::AppHost;
use crate::star::core::resource::host::artifact::ArtifactBundleHost;
use crate::star::core::resource::host::default::StatelessHost;
use crate::star::core::resource::host::mechtron::MechtronHost;
use crate::star::core::resource::host::space::SpaceHost;
use crate::star::StarSkel;
use crate::util::{AsyncProcessor, AsyncRunner, Call};
use crate::message::delivery::Delivery;
use crate::star::core::resource::host::kube::KubeHost;
use crate::star::core::resource::host::file::{FileHost, FileSystemHost};
use crate::html::{HTML, html_error_code};
use crate::frame::Reply;
use crate::star::core::message::WrappedHttpRequest;
use crate::mesh::serde::entity::request::{Http, Msg};
use crate::mesh::serde::resource::Resource;
use mesh_portal_api::message::Message;
use crate::mesh::serde::id::Address;

pub mod artifact;
mod default;
pub mod file_store;
pub mod kube;
mod space;
mod mechtron;
mod app;
mod file;

pub enum HostCall {
    Assign {
        assign: ResourceAssign<AssignResourceStateSrc>,
        tx: oneshot::Sender<Result<Resource, Error>>,
    },
    Select {
        key: ResourceKey,
        selector: ResourceHostPropertyValueSelector,
        tx: oneshot::Sender<Result<Option<ResourceValues<ResourceKey>>, Error>>,
    },
    Has {
        key: ResourceKey,
        tx: oneshot::Sender<bool>,
    },
    Handle(Delivery<Message>),
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
            HostCall::Select { key, selector, tx } => {
                let host = self.host(key.resource_type()).await;
                tx.send(host.select( key, selector).await);
            }
            HostCall::Assign { assign, tx } => {
                let host = self.host(assign.stub.key.resource_type()).await;
                match host.assign(assign.clone()).await {
                    Ok(state) => {
                        let resource = Resource::new(
                            assign.stub.key.clone(),
                            assign.stub.address.clone(),
                            assign.stub.archetype.clone(),
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
            HostCall::Handle(delivery) => {
                let host = self.host(key.resource_type() ).await;
                host.handle(delivery);
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
            ResourceType::ArtifactBundleSeries => Arc::new(StatelessHost::new(self.skel.clone(), ResourceType::ArtifactBundleSeries).await),
            ResourceType::ArtifactBundle=> Arc::new(ArtifactBundleHost::new(self.skel.clone()).await),
            ResourceType::App=> Arc::new(AppHost::new(self.skel.clone()).await),
            ResourceType::Mechtron => Arc::new(MechtronHost::new(self.skel.clone()).await),
            ResourceType::Database => Arc::new(KubeHost::new(self.skel.clone(), ResourceType::Database ).await.expect("KubeHost must be created without error")),
            ResourceType::FileSystem => Arc::new(FileSystemHost::new(self.skel.clone() ).await),
            ResourceType::File => Arc::new(FileHost::new(self.skel.clone()).await),

            t => unimplemented!("no HOST implementation for type {}", t.to_string()),
        };

        self.hosts.insert( rt, host.clone() );
        host
    }
}

#[async_trait]
pub trait Host: Send + Sync {

    fn resource_type(&self) -> ResourceType;


    async fn assign(
        &self,
        assign: ResourceAssign<AssignResourceStateSrc>,
    ) -> Result<(), Error>;

    fn handle( &self, delivery: Delivery<Message> ) {
        // delivery.fail(fail::Undeliverable)
    }

    async fn has(&self, address: Address) -> bool;

    fn shutdown(&self) {}

}
