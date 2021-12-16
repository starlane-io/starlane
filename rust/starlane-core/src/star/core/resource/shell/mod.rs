use std::collections::hash_map::RandomState;
use std::collections::HashMap;
use std::sync::Arc;

use k8s_openapi::kind;
use mesh_portal_api::message::Message;
use tokio::sync::{mpsc, oneshot};

use crate::error::Error;
use crate::fail::Fail;
use crate::html::{HTML, html_error_code};
use crate::mesh::{Request, Response};
use crate::mesh::serde::entity::request::{Http, Msg};
use crate::mesh::serde::fail;
use crate::mesh::serde::generic::payload::Payload;
use crate::mesh::serde::id::{Address, Kind};
use crate::mesh::serde::resource::Resource;
use crate::message::delivery::Delivery;
use crate::resource::{AssignResourceStateSrc, ResourceAssign, ResourceType};
use crate::star::core::message::WrappedHttpRequest;
use crate::star::core::resource::manager::{Host, ResourceManager};
use crate::star::core::resource::manager::app::AppManager;
use crate::star::core::resource::manager::artifact::ArtifactBundleManager;
use crate::star::core::resource::manager::file::{FileManager, FileSystemManager};
use crate::star::core::resource::manager::kube::KubeManager;
use crate::star::core::resource::manager::mechtron::MechtronManager;
use crate::star::core::resource::manager::stateless::StatelessManager;
use crate::star::StarSkel;
use crate::util::{AsyncProcessor, AsyncRunner, Call};


pub enum HostCall {
    Assign( Delivery<ResourceAssign> ),
    Has {
        address: Address,
        tx: oneshot::Sender<bool>,
    },
    Request (Delivery<Request>),
    Get(Delivery<Rc>)
}

impl Call for HostCall {}

pub struct HostComponent {
    skel: StarSkel,
    hosts: HashMap<ResourceType,Arc<dyn ResourceManager>>,
    resources: HashMap<Address,ResourceType>
}

impl HostComponent {
    pub fn new(skel: StarSkel) -> mpsc::Sender<HostCall> {
        let (tx, rx) = mpsc::channel(1024);
        AsyncRunner::new(Box::new(Self { skel, hosts: HashMap::new(), resources: HashMap::new() }), tx.clone(), rx);
        tx
    }
}

#[async_trait]
impl AsyncProcessor<HostCall> for HostComponent {
    async fn process(&mut self, call: HostCall) {
        match call {
            HostCall::Assign(assign) => {
                let stub = assign.item.stub.clone();
                match self.host(assign.item.stub.key.resource_type()).await
                {
                    Ok(host) => {
                        let result = host.assign(assign.clone().item).await;
                        if result.is_ok()
                        {
                            self.resources.insert( stub.address, stub.kind.resource_type() );
                        }
                        assign.ok(Payload::Empty);
                    }
                    Err(err) => {
                        // need to send a FAIL message if the delivery fails...
                        eprintln!("{}", err.to_string());
                    }
                }

            }
            HostCall::Has { address, tx } => {
                tx.send(self.resources.has(address).await);
            }
            HostCall::Request(delivery)=> {
                match self.resources.get(&delivery.to()) {
                    None => {
                        let fail = fail::Fail::Resource(fail::resource::Fail::BadRequest(fail::BadRequest::NotFound(fail::NotFound::Address(delivery.to().to_string()))));
                        delivery.fail(fail);
                    }
                    Some(resource_type) => {
                        match self.host(resource_type).await
                        {
                            Ok(host) => {
                                host.handle_request(delivery);
                            }
                            Err(err) => {
                                eprintln!("cannot find host for resource_type: {}", resource_type.to_string() );
                            }
                        }
                    }
                }
            }
        }
    }
}

impl HostComponent {
    async fn host(&mut self, rt: &ResourceType) -> Result<Arc<dyn ResourceManager>,Error> {

        if self.hosts.contains_key(rt) {
            Ok(self.hosts.get(rt).cloned().ok_or("expected reference to shell".into()));
        }

        let host: Arc<dyn ResourceManager> = match rt {
            ResourceType::Space => Arc::new(StatelessManager::new(self.skel.clone(), ResourceType::Space ).await),
            ResourceType::ArtifactBundleSeries => Arc::new(StatelessManager::new(self.skel.clone(), ResourceType::ArtifactBundleSeries).await),
            ResourceType::ArtifactBundle=> Arc::new(ArtifactBundleManager::new(self.skel.clone()).await),
            ResourceType::App=> Arc::new(AppManager::new(self.skel.clone()).await),
            ResourceType::Mechtron => Arc::new(MechtronManager::new(self.skel.clone()).await),
            ResourceType::Database => Arc::new(KubeManager::new(self.skel.clone(), ResourceType::Database ).await.expect("KubeHost must be created without error")),
            ResourceType::FileSystem => Arc::new(FileSystemManager::new(self.skel.clone() ).await),
            ResourceType::File => Arc::new(FileManager::new(self.skel.clone()).await),

            t => return Err(format!("no HOST implementation for type {}", t.to_string()).into()),
        };

        self.hosts.insert( rt.clone(), host.clone() );
        Ok(host)
    }
}
