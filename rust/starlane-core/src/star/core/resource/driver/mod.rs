use std::sync::Arc;

use tokio::sync::{mpsc, oneshot};

use artifact::ArtifactBundleCoreDriver;
use k8s::K8sCoreDriver;

use crate::error::Error;
use crate::message::delivery::Delivery;
use crate::{particle};
use crate::particle::{ParticleAssign, KindBase};
use crate::star::StarSkel;
use crate::util::{AsyncProcessor, Call, AsyncRunner};
use crate::star::core::resource::driver::stateless::StatelessCoreDriver;
use crate::star::core::resource::driver::mechtron::MechtronCoreDriver;
use crate::star::core::resource::driver::file::{FileSystemManager, FileCoreManager};
use std::collections::HashMap;
use std::future::Future;
use std::str::FromStr;
use mesh_portal::version::latest::entity::request::Rc;
use mesh_portal::version::latest::entity::request::set::Set;
use mesh_portal::version::latest::fail;
use mesh_portal::version::latest::id::Point;
use mesh_portal::version::latest::messaging::{Request, Response};
use mesh_portal::version::latest::payload::Payload;
use mesh_portal::version::latest::particle::Stub;
use mesh_portal_api_client::ResourceCommand;
use crate::star::core::resource::driver::artifact::ArtifactManager;
use crate::star::core::resource::driver::user::UserBaseKeycloakCoreDriver;

mod stateless;
pub mod artifact;
pub mod k8s;
pub mod mechtron;
pub mod file;
pub mod portal;
pub mod user;

#[derive(Clone)]
pub struct ResourceCoreDriverApi {
    pub tx: mpsc::Sender<ResourceManagerCall>,
}

impl ResourceCoreDriverApi {
    pub fn new(tx: mpsc::Sender<ResourceManagerCall>) -> Self {
        Self { tx }
    }

    pub async fn assign(&self, assign: ParticleAssign) -> Result<(),Error> {
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

    pub async fn get(&self, point: Point) -> Result<Payload,Error> {
        let (tx,rx) = oneshot::channel();
        self.tx.send(ResourceManagerCall::Get{point, tx }).await;
        rx.await?
    }

    pub async fn particle_command(&self, to: Point, rc: Rc) -> Result<Payload,Error> {
        let (tx,rx) = oneshot::channel();
        self.tx.send(ResourceManagerCall::ResourceCommand { to, rc,  tx }).await;
        rx.await?
    }
}

pub enum ResourceManagerCall {
    Assign{ assign: ParticleAssign, tx: oneshot::Sender<Result<(),Error>> },
    Request { request: Request, tx: oneshot::Sender<Result<Response,Error>>},
    Get{ point: Point, tx: oneshot::Sender<Result<Payload,Error>>},
    ResourceCommand { to: Point, rc: Rc, tx: oneshot::Sender<Result<Payload,Error>> }
}


impl Call for ResourceManagerCall {}



pub struct ResourceCoreDriverComponent {
    pub skel: StarSkel,
    drivers: HashMap<KindBase,Box<dyn ParticleCoreDriver>>,
    resources: HashMap<Point, KindBase>
}

impl ResourceCoreDriverComponent {
    pub async fn new( skel: StarSkel, tx: mpsc::Sender<ResourceManagerCall>, rx: mpsc::Receiver<ResourceManagerCall> ) {
        let mut component = Self {
            skel,
            drivers: HashMap::new(),
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
impl AsyncProcessor<ResourceManagerCall> for ResourceCoreDriverComponent {
    async fn process(&mut self, call: ResourceManagerCall) {
        match call {
            ResourceManagerCall::Assign { assign, tx } => {
                self.assign(assign,tx).await;
            }
            ResourceManagerCall::Request { request, tx } => {
                match self.resources.get(&request.to ) {
                    Some(resource_type) => {
                        match self.drivers.get(resource_type) {
                            Some(manager) => {
                                tx.send(Ok(manager.handle_request(request).await));
                            }
                            None => {
                                let message = format!("cannot find driver for '{}' for point '{}'" , resource_type.to_string(), request.to.to_string());
                                error!("{}",message);
                                request.fail(message.as_str());
                            }
                        };
                    }
                    None => {
                        let message = format!("driver does not contain particle '{}'" ,  request.to.to_string());
                        error!("{}",message);
                        request.fail(message.as_str());
                    }
                }
            }
            ResourceManagerCall::Get { point, tx } => {
                self.get(point,tx).await;
            }
            ResourceManagerCall::ResourceCommand { to, rc, tx } => {
                tx.send( self.resource_command(to, rc).await );
            }
        }
    }
}

impl ResourceCoreDriverComponent {

    async fn assign(&mut self, assign: ParticleAssign, tx: oneshot::Sender<Result<(),Error>> ) {

       async fn process(manager_component: &mut ResourceCoreDriverComponent, assign: ParticleAssign) -> Result<(),Error> {
           let resource_type = KindBase::from_str(assign.details.stub.kind.to_string().as_str())?;
           let manager:&mut Box<dyn ParticleCoreDriver> = manager_component.drivers.get_mut(&resource_type ).ok_or(format!("could not get driver for {}", resource_type.to_string()))?;
           manager_component.resources.insert(assign.details.stub.point.clone(), resource_type );
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


    async fn get(&mut self, point: Point, tx: oneshot::Sender<Result<Payload,Error>> ) {
        async fn process(manager : &mut ResourceCoreDriverComponent, point: Point) -> Result<Payload,Error> {
            let resource_type = manager.resource_type(&point )?;
            let manager = manager.drivers.get(&resource_type ).ok_or(format!("could not get driver for {}", resource_type.to_string()))?;
            manager.get(point).await
        }

        tx.send( process(self,point).await );
    }


    async fn request( &mut self, request: Request) -> Response {
        async fn process(manager: &mut ResourceCoreDriverComponent, request: Request) -> Result<Response,Error> {
            let resource_type = manager.resource_type(&request.to)?;
            let manager = manager.drivers.get(&resource_type ).ok_or(format!("could not get driver for {}", resource_type.to_string()))?;
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

    async fn resource_command(&mut self, to: Point, rc: Rc) -> Result<Payload,Error> {
        let resource_type = self.resources.get(&to ).ok_or(format!("could not find particle: {}", to.to_string()))?;
        let driver = self.drivers.get(resource_type).ok_or(format!("do not have a particle core driver for '{}' and StarKind '{}'", resource_type.to_string(), self.skel.info.kind.to_string() ))?;
        let result = driver.resource_command(to,rc).await;
        match &result {
            Ok(payload) => {
                info!("particle command payload: {:?}", payload);
            }
            Err(err) => {
                error!("{}",err.to_string())
            }
        }
        result
    }

    fn resource_type(&mut self, point:&Point) ->Result<KindBase,Error> {
        Ok(self.resources.get(point ).ok_or(Error::new("could not find particle") )?.clone())
    }

    async fn has(&mut self, point: Point, tx: mpsc::Sender<bool> ) {
        tx.send( self.resources.contains_key(&point)  );
    }

    async fn init(&mut self ) -> Result<(),Error>
    {
        for resource_type in self.skel.info.kind.hosted() {
            let manager: Box<dyn ParticleCoreDriver> = match resource_type.clone() {
                KindBase::Root => Box::new(StatelessCoreDriver::new(self.skel.clone(), KindBase::Root ).await),
                KindBase::User => Box::new(StatelessCoreDriver::new(self.skel.clone(), KindBase::User ).await),
                KindBase::Control => Box::new(StatelessCoreDriver::new(self.skel.clone(), KindBase::Control ).await),
                KindBase::Proxy=> Box::new(StatelessCoreDriver::new(self.skel.clone(), KindBase::Proxy).await),
                KindBase::Space => Box::new(StatelessCoreDriver::new(self.skel.clone(), KindBase::Space ).await),
                KindBase::Base => Box::new(StatelessCoreDriver::new(self.skel.clone(), KindBase::Base ).await),
                KindBase::ArtifactBundleSeries => Box::new(StatelessCoreDriver::new(self.skel.clone(), KindBase::ArtifactBundleSeries).await),
                KindBase::ArtifactBundle=> Box::new(ArtifactBundleCoreDriver::new(self.skel.clone()).await),
                KindBase::Artifact => Box::new(ArtifactManager::new(self.skel.clone()).await ),
                KindBase::App => Box::new(MechtronCoreDriver::new(self.skel.clone(), KindBase::App).await?),
                KindBase::Mechtron => Box::new(MechtronCoreDriver::new(self.skel.clone(), KindBase::Mechtron).await?),
                KindBase::Database => Box::new(K8sCoreDriver::new(self.skel.clone(), KindBase::Database ).await?),
                KindBase::FileSystem => Box::new(FileSystemManager::new(self.skel.clone() ).await),
                KindBase::File => Box::new(FileCoreManager::new(self.skel.clone())),
                KindBase::UserBase=> Box::new(UserBaseKeycloakCoreDriver::new(self.skel.clone()).await? ),
                t => Box::new(StatelessCoreDriver::new(self.skel.clone(), t ).await)
            };
            self.drivers.insert(resource_type, manager );
        }
        Ok(())
    }
}

#[async_trait]
pub trait ParticleCoreDriver: Send + Sync {

    fn kind(&self) -> particle::KindBase;

    async fn assign(
        &mut self,
        assign: ParticleAssign,
    ) -> Result<(),Error>;

    async fn handle_request(&self, request: Request ) -> Response {
        request.fail(format!("particle type '{}' does not handle requests",self.kind().to_string()).as_str())
    }

    async fn get(&self, point: Point) -> Result<Payload,Error> {
        Err("Stateless".into())
    }

    fn shutdown(&self) {}

    async fn resource_command(&self, to: Point, rc: Rc) -> Result<Payload,Error> {
        Err(format!("particle type: '{}' does not handle Core particle commands",self.kind().to_string()).into())
    }

}
