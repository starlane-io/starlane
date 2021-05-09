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


use crate::actor::{Actor, ActorKey};
use crate::app::{ApplicationStatus};
use crate::error::Error;
use crate::frame::{ActorMessage, AppCreate, AppMessage, StarMessage, StarMessagePayload, Watch, WatchInfo};
use crate::id::{Id, IdSeq};
use crate::star::{ActorCreate, StarCommand, StarKey, StarKind, StarManagerCommand, StarSkel};
use crate::core::server::{ServerStarCore, ServerStarCoreExt, ExampleServerStarCoreExt};
use std::marker::PhantomData;

pub mod server;

pub enum StarCoreCommand
{
    AppMessage(StarCoreAppMessage),
    Watch(Watch),
}

pub struct StarCoreAppMessage
{
    pub message: AppMessage,
    pub tx: oneshot::Sender<AppCommandResult>
}

pub enum AppCommandResult
{
    Ok,
    Actor(ActorKey),
    Error(String)
}


pub enum StarCoreMessagePayload
{
    AppCommand(AppMessage)
}

pub enum StarCoreExtKind
{
    None,
    Server(Box<dyn ServerStarCoreExt>)
}

pub enum CoreRunnerCommand
{
    Core(Box<dyn StarCore>),
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
      let (tx,mut rx) = mpsc::channel(1);
      thread::spawn( move || {
         let runtime = Runtime::new().unwrap();
         runtime.block_on( async move {
            while let Option::Some(CoreRunnerCommand::Core(mut core)) = rx.recv().await
            {
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

    pub async fn run( &self, core: Box<dyn StarCore> ) {
        self.tx.send( CoreRunnerCommand::Core(core) ).await;
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

    pub fn create(&self, skel: StarSkel, ext: StarCoreExtKind, core_rx: mpsc::Receiver<StarCoreCommand> ) -> Result<Box<dyn StarCore>,Error>
    {
        match skel.info.kind
        {
            StarKind::Server(_) => {
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
            StarKind::Server(_) => {
                StarCoreExtKind::Server( Box::new(ExampleServerStarCoreExt::new(skel.clone()) ) )
            }
            _ => StarCoreExtKind::None
        }
    }
}





