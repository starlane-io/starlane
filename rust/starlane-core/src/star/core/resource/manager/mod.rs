use std::sync::Arc;

use tokio::sync::{mpsc, oneshot};

use artifact::ArtifactBundleManager;
use k8s::K8sManager;

use crate::error::Error;
use crate::message::delivery::Delivery;
use crate::{resource};
use crate::resource::{ResourceAssign, ResourceType};
use crate::star::StarSkel;
use crate::util::{AsyncProcessor, Call, AsyncRunner};
use crate::star::core::resource::manager::stateless::StatelessManager;
use crate::star::core::resource::manager::mechtron::MechtronManager;
use crate::star::core::resource::manager::file::{FileSystemManager, FileManager};
use std::collections::HashMap;
use std::future::Future;
use std::str::FromStr;
use mesh_portal_serde::version::latest::fail;
use mesh_portal_serde::version::latest::id::Address;
use mesh_portal_serde::version::latest::messaging::{Request, Response};
use mesh_portal_serde::version::latest::payload::Payload;
use mesh_portal_versions::version::v0_0_1::id::Tks;
use crate::star::core::resource::manager::artifact::ArtifactManager;

mod stateless;
pub mod artifact;
pub mod k8s;
pub mod mechtron;
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

    pub async fn request( &self, request: Request) -> Result<Response,Error> {
        let (tx,rx) = oneshot::channel();
println!("Manager mod request....");
        self.tx.send(ResourceManagerCall::Request{request, tx }).await;
println!("Manager mod requesxt .. waiting" );
        let rtn = rx.await?;
println!("Manager mod RETURNING" );
        rtn
    }

    pub async fn get( &self, address: Address ) -> Result<Payload,Error> {
        let (tx,rx) = oneshot::channel();
        self.tx.send(ResourceManagerCall::Get{address, tx }).await;
        rx.await?
    }
}

pub enum ResourceManagerCall {
    Assign{ assign:ResourceAssign, tx: oneshot::Sender<Result<(),Error>> },
    Request { request: Request, tx: oneshot::Sender<Result<Response,Error>>},
    Get{ address: Address, tx: oneshot::Sender<Result<Payload,Error>>}
}


impl Call for ResourceManagerCall {}



pub struct ResourceManagerComponent {
    pub skel: StarSkel,
    managers: HashMap<ResourceType,Box<dyn ResourceManager>>,
    resources: HashMap<Address,ResourceType>
}

impl ResourceManagerComponent {
    pub async fn new( skel: StarSkel, tx: mpsc::Sender<ResourceManagerCall>, rx: mpsc::Receiver<ResourceManagerCall> ) {
        let mut component = Self {
            skel,
            managers: HashMap::new(),
            resources: HashMap::new()
        };
        match component.init().await {
            Ok(_) => {}
            Err(err) => {
                error!("{}",err.to_string());
            }
        }
        AsyncRunner::new(
        Box::new(component),tx, rx);
    }
}

#[async_trait]
impl AsyncProcessor<ResourceManagerCall> for ResourceManagerComponent{
    async fn process(&mut self, call: ResourceManagerCall) {
        match call {
            ResourceManagerCall::Assign { assign, tx } => {
                self.assign(assign,tx).await;
            }
            ResourceManagerCall::Request { request, tx } => {
                match self.resources.get(&request.to ) {
                    Some(resource_type) => {
                        match self.managers.get(resource_type) {
                            Some(manager) => {
                                tx.send(Ok(manager.handle_request(request).await));
                            }
                            None => {
                                let message = format!("cannot find manager for '{}' for address '{}'" , resource_type.to_string(), request.to.to_string());
                                error!("{}",message);
                                request.fail(message.as_str());
                            }
                        };
                    }
                    None => {
                        let message = format!("manager does not contain resource '{}'" ,  request.to.to_string());
                        error!("{}",message);
                        request.fail(message.as_str());
                    }
                }
            }
            ResourceManagerCall::Get { address, tx } => {
                self.get(address,tx).await;
            }
        }
    }
}

impl ResourceManagerComponent{

    async fn assign( &mut self, assign: ResourceAssign, tx: oneshot::Sender<Result<(),Error>> ) {

       async fn process( manager_component: &mut ResourceManagerComponent, assign: ResourceAssign) -> Result<(),Error> {
           let resource_type = ResourceType::from_str(assign.stub.kind.resource_type().as_str())?;
           let manager:&mut Box<dyn ResourceManager> = manager_component.managers.get_mut(&resource_type ).ok_or(format!("could not get manager for {}",resource_type.to_string()))?;
           manager_component.resources.insert( assign.stub.address.clone(), resource_type );
           manager.assign(assign).await
       }
       let result = process(self,assign).await;
        match &result {
            Ok(_) => {}
            Err(err) => {
                error!("Resource Assign Error: {}", err.to_string());
            }
        }
       tx.send( result );
    }


    async fn get( &mut self, address: Address, tx: oneshot::Sender<Result<Payload,Error>> ) {
        async fn process( manager : &mut ResourceManagerComponent, address: Address) -> Result<Payload,Error> {
            let resource_type = manager.resource_type(&address )?;
            let manager = manager.managers.get(&resource_type ).ok_or(format!("could not get manager for {}",resource_type.to_string()))?;
            manager.get(address).await
        }

        tx.send( process(self,address).await );
    }


    async fn request( &mut self, request: Request) -> Response {
        async fn process( manager: &mut ResourceManagerComponent, request: Request) -> Result<Response,Error> {
            let resource_type = manager.resource_type(&request.to)?;
            let manager = manager.managers.get(&resource_type ).ok_or(format!("could not get manager for {}",resource_type.to_string()))?;
            Ok(manager.handle_request(request).await)
        }

        match process(self, request.clone() ).await {
            Ok(response) => {
                response
            }
            Err(error) => {
                request.fail(error.to_string().as_str() )
            }
        }
    }

    fn resource_type(&mut self, address:&Address )->Result<ResourceType,Error> {
        Ok(self.resources.get(address ).ok_or(Error::new("could not find resource") )?.clone())
    }

    async fn has( &mut self, address: Address, tx: mpsc::Sender<bool> ) {
        tx.send( self.resources.contains_key(&address)  );
    }

    async fn init(&mut self ) -> Result<(),Error>
    {
        for resource_type in self.skel.info.kind.hosted() {
            let manager: Box<dyn ResourceManager> = match resource_type.clone() {
                ResourceType::Root => Box::new(StatelessManager::new(self.skel.clone(), ResourceType::Root ).await),
                ResourceType::User => Box::new(StatelessManager::new(self.skel.clone(), ResourceType::User ).await),
                ResourceType::Control => Box::new(StatelessManager::new(self.skel.clone(), ResourceType::Control ).await),
                ResourceType::Proxy=> Box::new(StatelessManager::new(self.skel.clone(), ResourceType::Proxy).await),
                ResourceType::Space => Box::new(StatelessManager::new(self.skel.clone(), ResourceType::Space ).await),
                ResourceType::Base => Box::new(StatelessManager::new(self.skel.clone(), ResourceType::Base ).await),
                ResourceType::ArtifactBundleSeries => Box::new(StatelessManager::new(self.skel.clone(), ResourceType::ArtifactBundleSeries).await),
                ResourceType::ArtifactBundle=> Box::new(ArtifactBundleManager::new(self.skel.clone()).await),
                ResourceType::Artifact => Box::new(ArtifactManager::new(self.skel.clone()).await ),
                ResourceType::App => Box::new(MechtronManager::new(self.skel.clone(), ResourceType::App).await?),
                ResourceType::Mechtron => Box::new(MechtronManager::new(self.skel.clone(), ResourceType::Mechtron).await?),
                ResourceType::Database => Box::new(K8sManager::new(self.skel.clone(), ResourceType::Database ).await?),
                ResourceType::FileSystem => Box::new(FileSystemManager::new(self.skel.clone() ).await),
                ResourceType::File => Box::new(FileManager::new(self.skel.clone())),
                t => Box::new(StatelessManager::new(self.skel.clone(), t ).await)
            };
            self.managers.insert( resource_type, manager );
        }
        Ok(())
    }
}

#[async_trait]
pub trait ResourceManager: Send + Sync {

    fn resource_type(&self) -> resource::ResourceType;

    async fn assign(
        &mut self,
        assign: ResourceAssign,
    ) -> Result<(),Error>;

    async fn handle_request(&self, request: Request ) -> Response {
        request.fail(format!("resource '{}' does not handle requests",self.resource_type().to_string()).as_str())
    }

    async fn get(&self, address: Address) -> Result<Payload,Error> {
        Err("Stateless".into())
    }

    fn shutdown(&self) {}

}
