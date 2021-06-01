use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::future::Future;
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


use crate::actor::{ActorKey, ResourceMessage, ResourceMessagePayload};
use crate::app::{ApplicationStatus, AppArchetype, AppMeta};
use crate::error::Error;
use crate::frame::{StarMessage, StarMessagePayload, Watch, WatchInfo, ServerAppPayload, AppPayload, ResourceHostAction, ResourceHostResult};
use crate::id::{Id, IdSeq};
use crate::star::{ActorCreate, StarCommand, StarKey, StarKind, StarVariantCommand, StarSkel, Request, LocalResourceLocation};
use crate::core::server::{ServerStarCore, ServerStarCoreExt, ExampleServerStarCoreExt};
use std::marker::PhantomData;
use crate::keys::{AppKey, ResourceKey};
use crate::artifact::{Artifact, ArtifactKey};
use crate::resource::{ResourceStub, ResourceInit, ResourceSrc, ResourceAssign, ResourceSliceAssign, HostedResourceStore, HostedResource, LocalHostedResource};
use crate::message::Fail;

pub mod server;
pub mod filestore;
pub mod space;

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
    IsHosting(ResourceKey),
    Assign(ResourceAssign),
    Message(ResourceMessage)
}

pub enum StarCoreResult{
    Ok,
    LocalLocation(LocalResourceLocation),
    MessageReply(ResourceMessagePayload)
}



pub struct StarCoreAppCommand
{
    pub app: AppKey,
    pub payload: StarCoreAppCommandPayload
}

pub enum StarCoreAppCommandPayload
{
    None,
    Assign(Request<ResourceAssign,()>),
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
    AppCommand(AppPayload)
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
    pub fn new()->Self
    {
      let factory = StarCoreFactory::new();
      let (tx,mut rx) = mpsc::channel(1);
      thread::spawn( move || {
         let runtime = Runtime::new().unwrap();
         runtime.block_on( async move {
            while let Option::Some(CoreRunnerCommand::Core{ skel, ext, rx }) = rx.recv().await
            {
               let core = factory.create(skel,ext,rx).await;
               tokio::spawn( async move {
                   core.run().await
               } );
            }
         } );
      } );

      CoreRunner
      {
          tx: tx
      }
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

    pub async fn create(&self, skel: StarSkel, ext: StarCoreExtKind, core_rx: mpsc::Receiver<StarCoreAction> ) -> StarCore2
    {
        StarCore2::new(skel, core_rx).await
/*        match skel.info.kind
        {
            StarKind::ActorHost => {
                if let StarCoreExtKind::Server(ext) = ext
                {
                    Ok(Box::new(ServerStarCore::new(skel, ext, core_rx)))
                }
                else
                {
                    Err("expected ServerCoreExt".into())
                }
            }
            _ => Ok(Box::new(InertStarCore::new()))
        }
 */
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
pub trait Host{
    async fn assign(&self, assign: ResourceAssign) -> Result<(),Fail>;
}



pub struct StarCore2{
    skel: StarSkel,
    rx: mpsc::Receiver<StarCoreAction>,
    resources: HostedResourceStore,
    host: Box<dyn Host>
}

impl StarCore2{

    pub async fn new(skel: StarSkel, rx: mpsc::Receiver<StarCoreAction>, host: Box<dyn Host>) -> Self {
        StarCore2{
            skel: skel,
            rx: rx,
            resources: HostedResourceStore::new().await,
            host: host
        }
    }

    pub async fn run(mut self){
        while let Option::Some(action) = self.rx.recv().await{
            action.tx.send( self.process(action.command).await);
        }
    }

    async fn process(&mut self, command: StarCoreCommand ) ->Result<StarCoreResult,Fail>{
        match command{
            StarCoreCommand::IsHosting(key) => {
                if self.resources.contains(&key).await? {
                    let local = LocalResourceLocation::new(key, Option::None );
                    Ok(StarCoreResult::LocalLocation(local))
                } else{
                    Err(Fail::ResourceNotFound(key))
                }
            }
            StarCoreCommand::Assign(assign) => {
                self.host.assign(assign).await?;
                Ok(StarCoreResult::Ok)
            }
            _ => {
                unimplemented!()
            }
        }
    }
}