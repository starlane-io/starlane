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
use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;
use tokio::time::Duration;


use crate::actor::{Actor, ActorKey};
use crate::app::ApplicationStatus;
use crate::error::Error;
use crate::frame::{ActorMessage, AppCreate, AppMessage, StarMessage, StarMessagePayload, Watch, WatchInfo};
use crate::id::{Id, IdSeq};
use crate::star::{ActorCommand, ActorCreate, StarCommand, StarKey, StarKind, StarManagerCommand, StarSkel};
use crate::core::server::{ServerStarCore, ServerStarCoreExt, ExampleServerStarCoreExt};

pub mod server;

pub enum StarCoreCommand
{
    StarExt(StarExt),
    StarSkel(StarSkel),
    Message(ActorMessage),
    Watch(Watch),
    Actor(ActorCommand)
}

pub enum StarExt
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
               tokio::spawn( async move { core.run().await } );
            }
         } );
         runtime.shutdown_background();
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
    async fn star_skel(&mut self, data: StarSkel);
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

    pub fn create( &self, kind: &StarKind ) -> (Box<dyn StarCore>,Sender<StarCoreCommand>)
    {
        let ( tx, rx ) = mpsc::channel(16);
        let core:Box<dyn StarCore> = match kind
        {
            StarKind::Server(_) => {
                Box::new(ServerStarCore::new(rx))
            }
            _ => Box::new(InertStarCore::new())
        };

        (core,tx)
    }
}

pub struct InertStarCore {
}

#[async_trait]
impl StarCore for InertStarCore
{
    async fn run(&mut self) {
        // do nothing
    }
}

impl InertStarCore {
    pub fn new()->Self {
        InertStarCore {}
    }
}

pub trait StarCoreExtFactory: Send+Sync
{
    fn create( &self, kind: &StarKind ) -> StarExt;
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
    fn create(&self, kind: &StarKind) -> StarExt {
        match kind
        {
            StarKind::Server(_) => {
                StarExt::Server( Box::new(ExampleServerStarCoreExt::new() ) )
            }
            _ => StarExt::None
        }
    }
}
