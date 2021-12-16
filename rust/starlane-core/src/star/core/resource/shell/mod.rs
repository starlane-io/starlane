use std::collections::hash_map::RandomState;
use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{mpsc, oneshot};

use crate::error::Error;
use crate::resource::{ResourceType, AssignResourceStateSrc, ResourceAssign};
use crate::star::core::resource::shell::app::AppHost;
use crate::star::core::resource::shell::artifact::ArtifactBundleHost;
use crate::star::core::resource::shell::default::StatelessHost;
use crate::star::core::resource::shell::mechtron::MechtronHost;
use crate::star::StarSkel;
use crate::util::{AsyncProcessor, AsyncRunner, Call};
use crate::message::delivery::Delivery;
use crate::star::core::resource::shell::kube::KubeHost;
use crate::star::core::resource::shell::file::{FileHost, FileSystemHost};
use crate::html::{HTML, html_error_code};
use crate::star::core::message::WrappedHttpRequest;
use crate::mesh::serde::entity::request::{Http, Msg};
use crate::mesh::serde::resource::Resource;
use mesh_portal_api::message::Message;
use crate::mesh::serde::id::{Address, Kind};
use crate::mesh::{Request, Response};

pub mod artifact;
mod default;
pub mod file_store;
pub mod kube;
mod mechtron;
mod app;
mod file;

pub enum HostCall {
    Assign {
        assign: ResourceAssign,
        tx: oneshot::Sender<Result<(), Error>>,
    },
    Has {
        address: Address,
        tx: oneshot::Sender<bool>,
    },
    Request {delivery: Delivery<Request>, kind: Kind },
    Response{response: Response, kind: Kind },
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
            HostCall::Has { address: key, tx } => {
                let host = self.host(key.resource_type()).await;
                tx.send(host.has(key).await);
            }
            HostCall::Request {delivery,kind }=> {
                let host = self.host(kind.resource_type() ).await;
                host.request(delivery);
            }
        }
    }
}

impl HostComponent {
    async fn host(&mut self, rt: ResourceType) -> Arc<dyn Host> {

        if self.hosts.contains_key(&rt) {
            return self.hosts.get(&rt).cloned().expect("expected reference to shell");
        }

        let host: Arc<dyn Host> = match rt {
            ResourceType::Space => Arc::new(StatelessHost::new(self.skel.clone(), ResourceType::Space ).await),
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
        assign: ResourceAssign,
    ) -> Result<(), Error>;

    fn request(&self, request: Delivery<Request> ) {
        // delivery.fail(fail::Undeliverable)
    }

    fn response(&self, response: Response ) {
        // delivery.fail(fail::Undeliverable)
    }


    async fn has(&self, address: Address) -> bool;

    fn shutdown(&self) {}

}
