use std::sync::Arc;

use tokio::sync::{mpsc, oneshot};

use artifact::ArtifactBundleManager;
use k8s::K8sManager;

use crate::error::Error;
use crate::message::delivery::Delivery;
use crate::{resource, fail};
use crate::resource::{ResourceAssign, ResourceType};
use crate::star::StarSkel;
use crate::util::{AsyncProcessor, Call, AsyncRunner};
use crate::star::core::resource::manager::stateless::StatelessManager;
use crate::star::core::resource::manager::app::AppManager;
use crate::star::core::resource::manager::mechtron::MechtronManager;
use crate::star::core::resource::manager::file::{FileSystemManager, FileManager};
use std::collections::HashMap;
use std::future::Future;
use std::str::FromStr;
use mesh_portal_serde::version::latest::id::Address;
use mesh_portal_serde::version::latest::messaging::{Request, Response};
use mesh_portal_serde::version::latest::payload::Payload;
use mesh_portal_versions::version::v0_0_1::id::Tks;

mod stateless;
pub mod artifact;
mod default;
pub mod file_store;
pub mod k8s;
pub mod mechtron;
pub mod app;
pub mod file;
pub mod portal;

#[derive(Clone)]
pub struct ResourceManagerApi {
    pub tx: mpsc::Sender<ResourceManagerCall>,
}

impl ResourceManagerApi {
    pub fn new(tx: mpsc::Sender<ResourceManagerCall>) -> Self {
        Self { tx }
    }

    pub async fn assign( &self, assign: ResourceAssign) -> Result<(),Error> {
        let (tx,rx) = oneshot::channel();
        self.tx.send(ResourceManagerCall::Assign{assign, tx }).await;
        rx.await?
    }

    pub async fn has( &self, address: Address) -> Result<bool,Error> {
        let (tx,rx) = oneshot::channel();
        self.tx.send(ResourceManagerCall::Has{address, tx }).await;
        Ok(rx.await?)
    }

    pub async fn request( &self, request: Delivery<Request>) {
        let (tx,rx) = oneshot::channel();
        self.tx.send(ResourceManagerCall::Request{request, tx }).await;
    }

    pub async fn get( &self, address: Address ) -> Result<Payload,Error> {
        let (tx,rx) = oneshot::channel();
        self.tx.send(ResourceManagerCall::Get{address, tx }).await;
        rx.await?
    }
}

pub enum ResourceManagerCall {
    Assign{ assign:ResourceAssign, tx: oneshot::Sender<Result<(),Error>> },
    Has { address: Address, tx: oneshot::Sender<bool> },
    Request { request: Delivery<Request>, tx: oneshot::Sender<Result<Option<Response>,Error>>},
    Get{ address: Address, tx: oneshot::Sender<Result<Payload,Error>>}
}


impl Call for ResourceManagerCall {}



pub struct ResourceManagerComponent {
    pub skel: StarSkel,
    managers: HashMap<ResourceType,Arc<dyn ResourceManager>>,
    resources: HashMap<Address,ResourceType>
}

impl ResourceManagerComponent {
    pub fn new( skel: StarSkel, tx: mpsc::Sender<ResourceManagerCall>, rx: mpsc::Receiver<ResourceManagerCall> ) {
        AsyncRunner::new(
        Box::new(Self {
            skel,
            managers: HashMap::new(),
            resources: HashMap::new()
        }),tx, rx);
    }
}

#[async_trait]
impl AsyncProcessor<ResourceManagerCall> for ResourceManagerComponent{
    async fn process(&mut self, call: ResourceManagerCall) {
        match call {
            ResourceManagerCall::Assign { assign, tx } => {
                self.assign(assign,tx).await;
            }
            ResourceManagerCall::Has { address, tx } => {}
            ResourceManagerCall::Request { request, tx } => {}
            ResourceManagerCall::Get { address, tx } => {}
        }
    }
}

impl ResourceManagerComponent{

    async fn assign( &mut self, assign: ResourceAssign, tx: oneshot::Sender<Result<(),Error>> ) {

       async fn process( manager_component: &mut ResourceManagerComponent, assign: ResourceAssign) -> Result<(),Error> {
           let resource_type = ResourceType::from_str(assign.stub.kind.resource_type().as_str())?;
           let manager = manager_component.manager(&resource_type ).await?;
           manager_component.resources.insert( assign.stub.address.clone(), resource_type );
           manager.assign(assign).await
       }

       tx.send( process(self,assign).await );
    }

    async fn request( &mut self, request: Delivery<Request>) {
        async fn process( manager: &mut ResourceManagerComponent, request: Delivery<Request>) -> Result<(),Error> {
            let resource_type = manager.resource_type(&request.to)?;
            let manager = manager.manager(&resource_type ).await?;
            manager.handle_request(request);
            Ok(())
        }

        match process(self, request.clone()).await {
            Ok(_) => {}
            Err(error) => {
                request.fail( mesh_portal_serde::version::latest::fail::Fail::Mesh(mesh_portal_serde::version::latest::fail::mesh::Fail::Error(error.to_string()) ))
            }
        }
    }

    fn resource_type(&mut self, address:&Address )->Result<ResourceType,Error> {
        Ok(self.resources.get(address ).ok_or(Error::new("could not find resource") )?.clone())
    }

    async fn has( &mut self, address: Address, tx: mpsc::Sender<bool> ) {
        tx.send( self.resources.contains_key(&address)  );
    }

    async fn manager(&mut self, rt: &ResourceType) -> Result<Arc<dyn ResourceManager>,Error> {

        if self.managers.contains_key(rt) {
            return Ok(self.managers.get(rt).cloned().expect("expected manager"));
        }

        let manager: Arc<dyn ResourceManager> = match rt {
            ResourceType::Space => Arc::new(StatelessManager::new(self.skel.clone(), ResourceType::Space ).await),
            ResourceType::Base=> Arc::new(StatelessManager::new(self.skel.clone(), ResourceType::Base ).await),
            ResourceType::ArtifactBundleSeries => Arc::new(StatelessManager::new(self.skel.clone(), ResourceType::ArtifactBundleSeries).await),
            ResourceType::ArtifactBundle=> Arc::new(ArtifactBundleManager::new(self.skel.clone()).await),
            ResourceType::App=> Arc::new(AppManager::new(self.skel.clone()).await),
            ResourceType::Mechtron => Arc::new(MechtronManager::new(self.skel.clone()).await),
            ResourceType::Database => Arc::new(K8sManager::new(self.skel.clone(), ResourceType::Database ).await.expect("K8sManager must be created without error")),
            ResourceType::FileSystem => Arc::new(FileSystemManager::new(self.skel.clone() ).await),
            ResourceType::File => Arc::new(FileManager::new(self.skel.clone())),

            t => return Err(format!("no Manager implementation for type {}", t.to_string()).into()),
        };

        self.managers.insert(rt.clone(), manager.clone() );
        Ok(manager)
    }
}

#[async_trait]
pub trait ResourceManager: Send + Sync {

    fn resource_type(&self) -> resource::ResourceType;


    async fn assign(
        &self,
        assign: ResourceAssign,
    ) -> Result<(),Error>;


    fn handle_request(&self, request: Delivery<Request> ) {
        // delivery.fail(fail::Undeliverable)
    }

    async fn has(&self, address: Address) -> bool;

    fn shutdown(&self) {}

}
