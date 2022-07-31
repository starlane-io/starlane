use std::sync::Arc;

use tokio::sync::{mpsc, oneshot};

use artifact::ArtifactBundleCoreDriver;
use k8s::K8sCoreDriver;

use crate::error::Error;
use crate::message::delivery::Delivery;
use crate::particle;
use crate::star::core::particle::driver::stateless::StatelessCoreDriver;
use crate::star::StarSkel;
use crate::util::{AsyncProcessor, AsyncRunner, Call};
//use crate::star::core::particle::driver::mechtron::MechtronCoreDriver;
use crate::star::core::particle::driver::artifact::ArtifactManager;
use crate::star::core::particle::driver::file::{FileCoreManager, FileSystemManager};
use crate::star::core::particle::driver::user::UserBaseKeycloakCoreDriver;
use cosmic_api::command::Command;
use cosmic_api::id::id::BaseKind;
use mesh_portal::version::latest::entity::request::set::Set;
use mesh_portal::version::latest::entity::request::Rc;
use mesh_portal::version::latest::fail;
use mesh_portal::version::latest::id::Point;
use mesh_portal::version::latest::messaging::{ReqShell, RespShell};
use mesh_portal::version::latest::particle::Stub;
use mesh_portal::version::latest::payload::Substance;
use mesh_portal::version::latest::sys::Assign;
use std::collections::HashMap;
use std::future::Future;
use std::str::FromStr;

pub mod artifact;
pub mod file;
pub mod k8s;
pub mod mechtron;
pub mod portal;
mod stateless;
pub mod user;

#[derive(Clone)]
pub struct ResourceCoreDriverApi {
    pub tx: mpsc::Sender<DriverCall>,
}

impl ResourceCoreDriverApi {
    pub fn new(tx: mpsc::Sender<DriverCall>) -> Self {
        Self { tx }
    }

    pub async fn assign(&self, assign: Assign) -> Result<(), Error> {
        let (tx, rx) = oneshot::channel();
        self.tx.send(DriverCall::Assign { assign, tx }).await;
        rx.await?
    }

    pub async fn request(&self, request: ReqShell) -> Result<RespShell, Error> {
        let (tx, rx) = oneshot::channel();
        println!("Manager mod request....");
        self.tx.send(DriverCall::Request { request, tx }).await;
        println!("Manager mod requesxt .. waiting");
        let rtn = rx.await?;
        println!("Manager mod RETURNING");
        rtn
    }

    pub async fn get(&self, point: Point) -> Result<Substance, Error> {
        let (tx, rx) = oneshot::channel();
        self.tx.send(DriverCall::Get { point, tx }).await;
        rx.await?
    }

    pub async fn command(&self, to: Point, command: Command) -> Result<Substance, Error> {
        let (tx, rx) = oneshot::channel();
        self.tx.send(DriverCall::Command { to, command, tx }).await;
        rx.await?
    }
}

pub enum DriverCall {
    Assign {
        assign: Assign,
        tx: oneshot::Sender<Result<(), Error>>,
    },
    Request {
        request: ReqShell,
        tx: oneshot::Sender<Result<RespShell, Error>>,
    },
    Get {
        point: Point,
        tx: oneshot::Sender<Result<Substance, Error>>,
    },
    Command {
        to: Point,
        command: Command,
        tx: oneshot::Sender<Result<Substance, Error>>,
    },
}

impl Call for DriverCall {}

pub struct ResourceCoreDriverComponent {
    pub skel: StarSkel,
    drivers: HashMap<BaseKind, Box<dyn ParticleCoreDriver>>,
    resources: HashMap<Point, BaseKind>,
}

impl ResourceCoreDriverComponent {
    pub async fn new(skel: StarSkel, tx: mpsc::Sender<DriverCall>, rx: mpsc::Receiver<DriverCall>) {
        let mut component = Self {
            skel,
            drivers: HashMap::new(),
            resources: HashMap::new(),
        };
        match component.init().await {
            Ok(_) => {}
            Err(err) => {
                error!("{}", err.to_string());
            }
        }
        AsyncRunner::new(Box::new(component), tx, rx);
    }
}

#[async_trait]
impl AsyncProcessor<DriverCall> for ResourceCoreDriverComponent {
    async fn process(&mut self, call: DriverCall) {
        match call {
            DriverCall::Assign { assign, tx } => {
                self.assign(assign, tx).await;
            }
            DriverCall::Request { request, tx } => match self.resources.get(&request.to) {
                Some(resource_type) => {
                    match self.drivers.get(resource_type) {
                        Some(manager) => {
                            tx.send(Ok(manager.handle_request(request).await));
                        }
                        None => {
                            let message = format!(
                                "cannot find driver for '{}' for point '{}'",
                                resource_type.to_string(),
                                request.to.to_string()
                            );
                            error!("{}", message);
                            request.fail(message.as_str());
                        }
                    };
                }
                None => {
                    let message = format!(
                        "driver does not contain particle '{}'",
                        request.to.to_string()
                    );
                    error!("{}", message);
                    request.fail(message.as_str());
                }
            },
            DriverCall::Get { point, tx } => {
                self.get(point, tx).await;
            }
            DriverCall::Command { to, command, tx } => {
                tx.send(self.particle_command(to, command).await);
            }
        }
    }
}

impl ResourceCoreDriverComponent {
    async fn assign(&mut self, assign: Assign, tx: oneshot::Sender<Result<(), Error>>) {
        async fn process(
            manager_component: &mut ResourceCoreDriverComponent,
            assign: Assign,
        ) -> Result<(), Error> {
            let resource_type = BaseKind::from_str(assign.details.stub.kind.to_string().as_str())?;
            let manager: &mut Box<dyn ParticleCoreDriver> = manager_component
                .drivers
                .get_mut(&resource_type)
                .ok_or(format!(
                    "could not get driver for {}",
                    resource_type.to_string()
                ))?;
            manager_component
                .resources
                .insert(assign.details.stub.point.clone(), resource_type);
            manager.assign(assign).await
        }
        let result = process(self, assign).await;
        match &result {
            Ok(_) => {}
            Err(err) => {
                error!("Resource Assign Error: {}", err.to_string());
            }
        }
        tx.send(result);
    }

    async fn get(&mut self, point: Point, tx: oneshot::Sender<Result<Substance, Error>>) {
        async fn process(
            manager: &mut ResourceCoreDriverComponent,
            point: Point,
        ) -> Result<Substance, Error> {
            let resource_type = manager.resource_type(&point)?;
            let manager = manager.drivers.get(&resource_type).ok_or(format!(
                "could not get driver for {}",
                resource_type.to_string()
            ))?;
            manager.get(point).await
        }

        tx.send(process(self, point).await);
    }

    async fn request(&mut self, request: ReqShell) -> RespShell {
        async fn process(
            manager: &mut ResourceCoreDriverComponent,
            request: ReqShell,
        ) -> Result<RespShell, Error> {
            let resource_type = manager.resource_type(&request.to)?;
            let manager = manager.drivers.get(&resource_type).ok_or(format!(
                "could not get driver for {}",
                resource_type.to_string()
            ))?;
            Ok(manager.handle_request(request).await)
        }

        match process(self, request.clone()).await {
            Ok(response) => response,
            Err(error) => request.fail(error.to_string().as_str()),
        }
    }

    async fn particle_command(&mut self, to: Point, command: Command) -> Result<Substance, Error> {
        let resource_type = self
            .resources
            .get(&to)
            .ok_or(format!("could not find particle: {}", to.to_string()))?;
        let driver = self.drivers.get(resource_type).ok_or(format!(
            "do not have a particle core driver for '{}' and StarKind '{}'",
            resource_type.to_string(),
            self.skel.info.kind.to_string()
        ))?;
        let result = driver.particle_command(to, command).await;
        match &result {
            Ok(payload) => {
                info!("particle command payload: {:?}", payload);
            }
            Err(err) => {
                error!("{}", err.to_string())
            }
        }
        result
    }

    fn resource_type(&mut self, point: &Point) -> Result<BaseKind, Error> {
        Ok(self
            .resources
            .get(point)
            .ok_or(Error::new("could not find particle"))?
            .clone())
    }

    async fn has(&mut self, point: Point, tx: mpsc::Sender<bool>) {
        tx.send(self.resources.contains_key(&point));
    }

    async fn init(&mut self) -> Result<(), Error> {
        for resource_type in self.skel.info.kind.hosted() {
            let manager: Box<dyn ParticleCoreDriver> = match resource_type.clone() {
                BaseKind::Root => {
                    Box::new(StatelessCoreDriver::new(self.skel.clone(), BaseKind::Root).await)
                }
                BaseKind::User => {
                    Box::new(StatelessCoreDriver::new(self.skel.clone(), BaseKind::User).await)
                }
                BaseKind::Control => {
                    Box::new(StatelessCoreDriver::new(self.skel.clone(), BaseKind::Control).await)
                }
                BaseKind::Space => {
                    Box::new(StatelessCoreDriver::new(self.skel.clone(), BaseKind::Space).await)
                }
                BaseKind::Base => {
                    Box::new(StatelessCoreDriver::new(self.skel.clone(), BaseKind::Base).await)
                }
                BaseKind::BundleSeries => Box::new(
                    StatelessCoreDriver::new(self.skel.clone(), BaseKind::BundleSeries).await,
                ),
                BaseKind::Bundle => {
                    Box::new(ArtifactBundleCoreDriver::new(self.skel.clone()).await)
                }
                BaseKind::Artifact => Box::new(ArtifactManager::new(self.skel.clone()).await),
                //                KindBase::App => Box::new(MechtronCoreDriver::new(self.skel.clone(), KindBase::App).await?),
                //                KindBase::Mechtron => Box::new(MechtronCoreDriver::new(self.skel.clone(), KindBase::Mechtron).await?),
                BaseKind::Database => {
                    Box::new(K8sCoreDriver::new(self.skel.clone(), BaseKind::Database).await?)
                }
                BaseKind::FileSystem => Box::new(FileSystemManager::new(self.skel.clone()).await),
                BaseKind::File => Box::new(FileCoreManager::new(self.skel.clone())),
                BaseKind::UserBase => {
                    Box::new(UserBaseKeycloakCoreDriver::new(self.skel.clone()).await?)
                }
                t => Box::new(StatelessCoreDriver::new(self.skel.clone(), t).await),
            };
            self.drivers.insert(resource_type, manager);
        }
        Ok(())
    }
}

#[async_trait]
pub trait ParticleCoreDriver: Send + Sync {
    fn kind(&self) -> BaseKind;

    async fn assign(&mut self, assign: Assign) -> Result<(), Error>;

    async fn handle_request(&self, request: ReqShell) -> RespShell {
        request.fail(
            format!(
                "particle type '{}' does not handle requests",
                self.kind().to_string()
            )
            .as_str(),
        )
    }

    async fn get(&self, point: Point) -> Result<Substance, Error> {
        Err("Stateless".into())
    }

    fn shutdown(&self) {}

    async fn particle_command(&self, to: Point, command: Command) -> Result<Substance, Error> {
        Err(format!(
            "particle type: '{}' does not handle Core particle commands",
            self.kind().to_string()
        )
        .into())
    }
}
