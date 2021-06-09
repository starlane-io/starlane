use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::marker::PhantomData;
use std::ops::Deref;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::thread;

use futures::future::BoxFuture;
use futures::FutureExt;
use tokio::runtime::Runtime;
use tokio::sync::{mpsc, oneshot};
use tokio::sync::mpsc::Sender;
use tokio::time::Duration;

use crate::actor::{ActorKey };
use crate::app::{AppArchetype, ApplicationStatus, AppMeta};
use crate::artifact::{Artifact, ArtifactKey};
use crate::core::server::{ExampleServerStarCoreExt, ServerStarCore, ServerStarCoreExt};
use crate::core::space::SpaceHost;
use crate::error::Error;
use crate::frame::{ResourceHostAction, ResourceHostResult, ServerAppPayload, StarMessage, StarMessagePayload, Watch, WatchInfo, MessagePayload};
use crate::id::{Id, IdSeq};
use crate::keys::{AppKey, ResourceKey};
use crate::message::Fail;
use crate::resource::{AssignResourceStateSrc, HostedResource, HostedResourceStore, LocalHostedResource, Resource, ResourceAssign, ResourceInit, ResourceSliceAssign};
use crate::resource::store::ResourceStoreSqlLite;
use crate::star::{ActorCreate, LocalResourceLocation, Request, StarCommand, StarKey, StarKind, StarSkel, StarVariantCommand};
use crate::file::FileAccess;
use crate::core::file_store::FileStoreHost;

pub mod server;
pub mod space;
pub mod file_store;

pub struct StarCoreAction{
    pub command: StarCoreCommand,
    pub tx: oneshot::Sender<Result<StarCoreResult,Fail>>
}

impl StarCoreAction{
    pub fn new( command: StarCoreCommand )-> (Self,oneshot::Receiver<Result<StarCoreResult,Fail>>){
        let (tx,rx) = oneshot::channel();
        (StarCoreAction{
            command: command,
            tx: tx
        },rx)
    }
}

pub enum StarCoreCommand
{
    Get(ResourceKey),
    Assign(ResourceAssign<AssignResourceStateSrc>),
}

pub enum StarCoreResult{
    Ok,
    Resource(Option<Resource>),
    LocalLocation(LocalResourceLocation),
    MessageReply(MessagePayload)
}

impl ToString for StarCoreResult{
    fn to_string(&self) -> String {
        match self{
            StarCoreResult::Ok => "Ok".to_string(),
            StarCoreResult::LocalLocation(_) => "LocalLocation".to_string(),
            StarCoreResult::MessageReply(_) => "MessageReply".to_string(),
            StarCoreResult::Resource(_) => "Resource".to_string()
        }
    }
}



pub struct StarCoreAppCommand
{
    pub app: AppKey,
    pub payload: StarCoreAppCommandPayload
}

pub enum StarCoreAppCommandPayload
{
    None,
    Assign(Request<ResourceAssign<AssignResourceStateSrc>,()>),
    AssignSlice(Request<ResourceSliceAssign,()>),
    InitSlice(Request<ResourceInit,()>)
}

pub enum AppLaunchError
{
   Error(String)
}

pub enum AppCommandResult
{
    Ok,
    Actor(ResourceKey),
    Error(String)
}


pub enum StarCoreMessagePayload
{
}

pub enum StarCoreExtKind
{
    None,
    Server(Box<dyn ServerStarCoreExt>)
}

pub enum CoreRunnerCommand
{
    Core{
        skel: StarSkel,
        ext: StarCoreExtKind,
        rx: mpsc::Receiver<StarCoreAction>
    },
    Shutdown
}

pub struct CoreRunner
{
    tx: mpsc::Sender<CoreRunnerCommand>
}

impl CoreRunner
{
    pub fn new()->Result<Self,Error>
    {
      let factory = StarCoreFactory::new();
      let (tx,mut rx) = mpsc::channel(1);
      thread::spawn( move || {
         let runtime = Runtime::new().unwrap();
         runtime.block_on( async move {
            while let Option::Some(CoreRunnerCommand::Core{ skel, ext, rx }) = rx.recv().await
            {
               let core = match factory.create(skel,ext,rx).await{
                   Ok(core) => core,
                   Err(err) => {
                       eprintln!("FATAL: {}", err);
                       std::process::exit(1);
                   }
               };
               tokio::spawn( async move {
                   core.run().await
               } );
            }
         } );
      } );

      Ok(CoreRunner
      {
          tx: tx
      })
    }

    pub async fn send( &self, command: CoreRunnerCommand ) {
        self.tx.send( command ).await;
    }
}


#[async_trait]
pub trait StarCoreExt: Sync+Send
{
}

#[async_trait]
pub trait StarCore: Sync+Send
{
    async fn run(&mut self);
}

pub struct StarCoreFactory
{

}

impl StarCoreFactory
{
    pub fn new()->Self
    {
        StarCoreFactory{}
    }

    pub async fn create(&self, skel: StarSkel, ext: StarCoreExtKind, core_rx: mpsc::Receiver<StarCoreAction> ) -> Result<StarCore2,Error>
    {
        let file_access = skel.file_access.with_path(format!("stars/{}",skel.info.key.to_string())).await?;
        let host:Box<dyn Host> =  match skel.info.kind
        {
            StarKind::SpaceHost => {
                Box::new(SpaceHost::new().await)
            }
            StarKind::FileStore => {
                Box::new(FileStoreHost::new(skel.clone(),file_access).await? )
            }
            _ => {
                Box::new(InertHost::new())
            }
        };
        Ok(StarCore2::new(skel, core_rx, host ).await)
    }
}

pub struct InertHost{
}

impl InertHost{
    pub fn new() -> Self {
        InertHost {}
    }
}

#[async_trait]
impl Host for InertHost{
    async fn assign(&mut self, assign: ResourceAssign<AssignResourceStateSrc>) -> Result<Resource, Fail> {
        Err(Fail::WrongResourceType {expected:HashSet::new(),received:assign.stub.archetype.kind.resource_type()})
    }

    async fn get(&self, key: ResourceKey) -> Result<Option<Resource>, Fail> {
        Err(Fail::WrongResourceType {expected:HashSet::new(),received:key.resource_type()})
    }
}

pub struct InertStarCore {
}

#[async_trait]
impl StarCore for InertStarCore
{
    async fn run(&mut self){
    }
}

impl InertStarCore {
    pub fn new()->Self {
        InertStarCore {}
    }
}

pub trait StarCoreExtFactory: Send+Sync
{
    fn create( &self, skell: &StarSkel ) -> StarCoreExtKind;
}

pub struct ExampleStarCoreExtFactory
{
}

impl ExampleStarCoreExtFactory
{
    pub fn new()->Self
    {
        ExampleStarCoreExtFactory{}
    }
}

impl StarCoreExtFactory for ExampleStarCoreExtFactory
{
    fn create(&self, skel: &StarSkel ) -> StarCoreExtKind {
        match skel.info.kind
        {
            StarKind::ActorHost => {
                StarCoreExtKind::Server( Box::new(ExampleServerStarCoreExt::new(skel.clone()) ) )
            }
            _ => StarCoreExtKind::None
        }
    }
}


#[async_trait]
pub trait Host: Send+Sync{
    async fn assign(&mut self, assign: ResourceAssign<AssignResourceStateSrc>) -> Result<Resource,Fail>;
    async fn get(&self, key: ResourceKey) -> Result<Option<Resource>,Fail>;
}



pub struct StarCore2{
    skel: StarSkel,
    rx: mpsc::Receiver<StarCoreAction>,
    host: Box<dyn Host>
}

impl StarCore2{

    pub async fn new(skel: StarSkel, rx: mpsc::Receiver<StarCoreAction>, host: Box<dyn Host>) -> Self {
        StarCore2{
            skel: skel,
            rx: rx,
            host: host
        }
    }

    pub async fn run(mut self){
        while let Option::Some(action) = self.rx.recv().await{
            let result = self.process(action.command).await;
            if action.tx.send( result ).is_err() {
                println!("Warning: Core sent response but got error.");
            }
        }
    }

    async fn process(&mut self, command: StarCoreCommand ) ->Result<StarCoreResult,Fail>{
        match command{

            StarCoreCommand::Assign(assign) => {
println!("CORE... RETURNING RESOURCE");
                Ok(StarCoreResult::Resource(Option::Some(self.host.assign(assign).await?)))
            }
            StarCoreCommand::Get(key) => {
                let resource = self.host.get(key).await?;
                Ok(StarCoreResult::Resource(resource))
            }
            _ => unimplemented!()
        }
    }
}