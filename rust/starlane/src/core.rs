use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::mpsc;

use crate::application::ApplicationStatus;
use crate::error::Error;
use crate::frame::{ActorMessage, StarMessage, StarMessagePayload, StarUnwindPayload, StarWind, Watch, WatchInfo};
use crate::id::{Id, IdSeq};
use crate::actor::{ActorKey, Actor};
use crate::star::{StarCommand, StarKey, StarKind, EntityCommand, EntityCreate};
use std::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;
use tokio::runtime::Runtime;
use tokio::time::Duration;
use std::future::Future;


pub enum StarCoreCommand
{
    Message(ActorMessage),
    Watch(Watch),
    Entity(EntityCommand)
}


pub struct CoreRunner
{
    runtime: Option<Runtime>,
    factory: Box<dyn StarCoreFactory>
}

impl CoreRunner
{
    pub fn start(&mut self)
    {
        if self.runtime.is_some()
        {
            return;
        }

        self.runtime = Option::new(Runtime::new().unwrap());
    }

    pub fn stop(&mut self)
    {
        if let Some(runtime) = &self.runtime
        {
            runtime.shutdown_timeout(Duration::from_secs(15));
            self.runtime = Option::None;
        }
    }

    fn run(&mut self, future: Box<dyn Future<Output=()>>) ->Result<(),Error>
    {
        if let Some(runtime)=&mut self.runtime
        {
            let runtime = self.runtime.unwrap();
            runtime.spawn(future);
            Ok(())
        }
        else {
            Err("CoreRunner: runtime has not been started.".into())
        }
    }

    pub fn create(&mut self, kind: &StarKind )->mpsc::Sender<StarCoreCommand>
    {
        let (tx,rx) = mpsc::channel(32);
        self.factory.create(kind,tx)
    }
}


pub trait StarCoreExt: Sync+Send
{
}

pub trait EntityStarCoreExt: StarCoreExt
{
    fn create_entity( &mut self, create: EntityCreate ) -> Result<ActorKey,Error>;
    fn message(&mut self, message: ActorMessage) -> Result<(),Error>;
    fn watch( &mut self, watch: Watch ) -> Result<(),Error>;
}


#[async_trait]
pub trait StarCoreFactory: Sync+Send
{
    fn create( &self, kind: &StarKind, star_tx: mpsc::Sender<StarCommand> ) -> mpsc::Sender<StarCoreCommand>;
}

pub struct StarCoreFactoryDefault
{
}

#[async_trait]
impl StarCoreFactory for StarCoreFactoryDefault
{
    fn create(&self, kind: &StarKind, star_tx: mpsc::Sender<StarCommand>) -> Sender<StarCoreCommand> {
       let (tx,rx) = mpsc::channel(32);
       let mut core = ServerStarCore{
           command_rx: rx
       };

      // instance runtime here

       tx
    }
}



pub struct ServerStarCore
{
    command_rx: mpsc::Receiver<StarCoreCommand>
}

impl ServerStarCore
{

    async fn run(&mut self)
    {
        while let Option::Some(command) = self.command_rx.recv().await
        {
            match &command
            {
                StarCoreCommand::Message(_) => {}
                StarCoreCommand::Watch(_) => {}
                StarCoreCommand::Entity(entity_command) => {
                    match entity_command
                    {
                        EntityCommand::Create(create) => {

                            // need to communicate with Ext here...

                        }
                    }
                }
            }
        }
    }
}