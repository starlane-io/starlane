use std::collections::hash_map::RandomState;
use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{mpsc, oneshot};

use starlane_resources::{AssignKind, AssignResourceStateSrc, Resource, ResourceAssign, ResourcePathAndType};
use starlane_resources::data::{BinSrc, DataSet};
use starlane_resources::message::{Fail, ResourcePortMessage, Message};

use crate::error::Error;
use crate::resource::{ResourceKey, ResourceType};
use crate::star::core::resource::host::app::AppHost;
use crate::star::core::resource::host::artifact::ArtifactBundleHost;
use crate::star::core::resource::host::default::StatelessHost;
use crate::star::core::resource::host::mechtron::MechtronHost;
use crate::star::core::resource::host::space::SpaceHost;
use crate::star::StarSkel;
use crate::util::{AsyncProcessor, AsyncRunner, Call};
use crate::message::resource::Delivery;
use crate::star::core::resource::host::kube::KubeHost;
use crate::star::core::resource::host::file::{FileHost, FileSystemHost};
use starlane_resources::property::{ResourceValueSelector, ResourceValues, ResourcePropertyValueSelector, ResourceValue, ResourceHostPropertyValueSelector};
use starlane_resources::status::Status;
use starlane_resources::http::HttpRequest;
use crate::html::{HTML, html_error_code};
use crate::frame::Reply;
use crate::star::core::message::WrappedHttpRequest;

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
        assign: ResourceAssign<AssignResourceStateSrc<DataSet<BinSrc>>>,
        tx: oneshot::Sender<Result<Resource, Error>>,
    },
    Init{
        key: ResourceKey,
        tx: oneshot::Sender<Result<(), Error>>,
    },
    UpdateState {
        key: ResourceKey,
        state: DataSet<BinSrc>,
        tx: oneshot::Sender<Result<(),Error>>
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
    Port(Delivery<Message<ResourcePortMessage>>),
    Http(Delivery<Message<HttpRequest>>),
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
            HostCall::Init { key, tx } => {
                let host = self.host(key.resource_type()).await;
                tx.send(host.init(key).await);
            }
            HostCall::UpdateState { key, state, tx }  => {
                let host = self.host(key.resource_type()).await;
                tx.send(host.update_state(key, state).await);
            }
            HostCall::Has { key, tx } => {
                let host = self.host(key.resource_type()).await;
                tx.send(host.has(key).await);
            }
            HostCall::Port(delivery) => {
                match self.skel.resource_locator_api.as_key( delivery.payload.to.clone() ).await
                {
                    Ok(key) => {
                        let host = self.host(key.resource_type()).await;
                        host.port_message(key, delivery).await;
                    }
                    Err(_) => {
                        error!("could not find key for: {}", delivery.payload.to.to_string() );
                    }

                }
            }
            HostCall::Http(delivery) => {
                match self.skel.resource_locator_api.as_key( delivery.payload.to.clone() ).await
                {
                    Ok(key) => {
                        let host = self.host(key.resource_type()).await;
                        host.http_message(key, delivery).await;
                    }
                    Err(_) => {
                        error!("could not find key for: {}", delivery.payload.to.to_string() );
                        delivery.fail( Fail::Error(format!("could not find key for: {}", delivery.payload.to.to_string())));
                    }

                }
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
        assign: ResourceAssign<AssignResourceStateSrc<DataSet<BinSrc>>>,
    ) -> Result<DataSet<BinSrc>, Error>;


    async fn init(&self, key: ResourceKey ) -> Result<(),Error> {
        Ok(())
    }
    async fn has(&self, key: ResourceKey) -> bool;
//    async fn select(&self, key: ResourceKey, selector: ResourcePropertyValueSelector ) -> Result<Option<ResourceValues<ResourceKey>>, Error>;
    async fn delete(&self, key: ResourceKey) -> Result<(), Error>;

    async fn get_state(&self,key: ResourceKey) -> Result<Option<DataSet<BinSrc>>,Error>;

    async fn update_state(&self,key: ResourceKey, state: DataSet<BinSrc> ) -> Result<(),Error> {
        Err(format!("resource type: {} does not allow state updates", key.resource_type().to_string()).into() )
    }

    async fn port_message(&self, key: ResourceKey, delivery: Delivery<Message<ResourcePortMessage>>) -> Result<(),Error>{
        info!("ignoring delivery");
        Ok(())
    }


    async fn http_message(&self, key: ResourceKey, delivery: Delivery<Message<HttpRequest>>) -> Result<(),Error>{
        eprintln!("resource does not respond to HttpRequest: <{}>", self.resource_type().to_string() );
        let response = html_error_code(400, "BAD REQUEST".to_string(), format!("This type of resource: <{}> cannot respond to http requests", self.resource_type().to_string()  ) )?;
        delivery.reply(Reply::HttpResponse(response));
        Ok(())
    }


    fn shutdown(&self) {}

    async fn select(&self, key: ResourceKey, selector: ResourceHostPropertyValueSelector) -> Result<Option<ResourceValues<ResourceKey>>, Error> {
        match &selector {
            ResourceHostPropertyValueSelector::State { aspect, field } => {
                let state = self.get_state(key.clone()).await?.unwrap_or(DataSet::new());
                let state = aspect.filter(state);
                let mut values = HashMap::new();
                values.insert(selector.into(), state );
                Ok(Option::Some(ResourceValues{
                    resource: key,
                    values
                }))
            }
        }
    }

}
