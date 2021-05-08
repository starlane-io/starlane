use tokio::sync::mpsc;

use crate::actor::ActorKey;
use crate::app::Alert;
use crate::core::{StarCore, StarCoreCommand, StarCoreExt, StarExt};
use crate::error::Error;
use crate::frame::{ActorMessage, AppCreate, AppMessage, Watch};
use crate::star::{ActorCommand, ActorCreate, StarSkel};

pub struct ServerStarCore
{
    pub skel: Option<StarSkel>,
    pub ext: Option<Box<dyn ServerStarCoreExt>>,
    pub core_rx: mpsc::Receiver<StarCoreCommand>
}

impl ServerStarCore
{
    pub fn new(core_rx: mpsc::Receiver<StarCoreCommand>)->Self
    {
        ServerStarCore
        {
            skel: Option::None,
            ext: Option::None,
            core_rx: core_rx
        }
    }
}

impl ServerStarCore
{
    pub async fn alert( &mut self, alert: Alert )
    {
        // not sure what to do with alerts yet
    }
}
#[async_trait]
impl StarCore for ServerStarCore
{
    async fn run(&mut self)
    {
        while let Option::Some(command) = self.core_rx.recv().await
        {
            match command
            {
                StarCoreCommand::StarExt(ext) => {
                    if let StarExt::Server(ext) = ext
                    {
                        self.ext = Option::Some(ext);
                    }
                }
                StarCoreCommand::StarSkel(skel) => {
                    self.skel = Option::Some(skel.clone());
                    match &mut self.ext
                    {
                        None => {
                            // fire alert
                            self.alert( Alert::Yellow("ServerStarCore: expected to receive StarExt before StarSkel".into())).await;
                        }
                        Some(ext) => {
                            ext.star_skel(skel ).await;
                        }
                    }
                }
                StarCoreCommand::Message(_) => {}
                StarCoreCommand::Watch(_) => {}
                StarCoreCommand::Actor(actor_command) => {
                    match actor_command
                    {
                        ActorCommand::Create(create) => {

                            // need to communicate with Ext here...

                        }
                    }
                }
            }
        }
    }
}

#[async_trait]
pub trait ServerStarCoreExt: StarCoreExt
{
    async fn actor_create(&self, create: ActorCreate) -> ActorCreateResult;
    async fn actor_message(&self, message: ActorMessage) -> ActorMessageResult;
    async fn app_create(&self, message: AppCreate )->Result<(),Error>;
    async fn app_message(&self, message: AppMessage ) -> AppMessageResult;
    async fn watch( &self, watch: Watch );
}

pub enum ActorCreateResult
{
    Ok(ActorKey),
    Error(String)
}

pub enum ActorMessageResult
{
    Ok,
    ActorNotPresent,
    Error(String)
}

pub enum AppMessageResult
{
    Ok,
    AppNotPresent,
    AppNotReady,
    Error(String)
}


pub struct ExampleServerStarCoreExt
{
    pub skel: Option<StarSkel>,
}

impl ExampleServerStarCoreExt
{
    pub fn new()->Self
    {
       ExampleServerStarCoreExt{
           skel: Option::None
       }
    }
}

#[async_trait]
impl StarCoreExt for ExampleServerStarCoreExt
{
    async fn star_skel(&mut self, skel: StarSkel) {
        self.skel = Option::Some(skel);
    }
}

#[async_trait]
impl ServerStarCoreExt for ExampleServerStarCoreExt
{
    async fn actor_create(&self, create: ActorCreate) -> ActorCreateResult {
        todo!()
    }

    async fn actor_message(&self, message: ActorMessage) -> ActorMessageResult {
        todo!()
    }

    async fn app_create(&self, message: AppCreate) -> Result<(), Error> {
        todo!()
    }

    async fn app_message(&self, message: AppMessage) -> AppMessageResult {
        todo!()
    }

    async fn watch(&self, watch: Watch) {
        todo!()
    }
}