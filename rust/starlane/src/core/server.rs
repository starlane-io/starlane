use tokio::sync::mpsc;

use crate::actor::ActorKey;
use crate::app::{Alert, AppCommandKind};
use crate::core::{StarCore, StarCoreCommand, StarCoreExt, StarExt, AppCommandResult};
use crate::error::Error;
use crate::frame::{ActorMessage, AppCreate, AppMessage, Watch, AppMessagePayload};
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
                StarCoreCommand::Watch(_) => {}

                StarCoreCommand::AppMessage(message) => {
                    match message.message.payload
                    {
                        AppMessagePayload::None => {}
                        AppMessagePayload::Launch(create) => {
                            match &self.ext
                            {
                                None => {
                                    // fire alert
                                    self.alert( Alert::Red("ServerStarCore: cannot launch app because StarExt is None".into())).await;
                                }
                                Some(ext) => {
                                    let result = ext.app_create(AppCreate{
                                        app: message.message.app.clone(),
                                        data: create
                                    }).await;
                                    message.tx.send(result);
                                }
                            }
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
    async fn app_create(&self, message: AppCreate )-> AppCommandResult;
    async fn app_message(&self, message: AppMessage ) -> AppCommandResult;
    async fn actor_message(&self, message: ActorMessage) -> AppCommandResult;
    async fn watch( &self, watch: Watch );
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
    async fn app_create(&self, create: AppCreate) -> AppCommandResult {
        println!("ExampleServer AppCreate");
        match create.data.kind.as_str()
        {
            "test" => {
println!("creation of the test app...");
                AppCommandResult::Ok
            }
            unexpected => {
println!("donot know how to create: {}",unexpected);
                AppCommandResult::Error(format!("do not know how to create app kind {}",unexpected).into())
            }
        }
    }

    async fn actor_message(&self, message: ActorMessage) -> AppCommandResult{
        todo!()
    }

    async fn app_message(&self, message: AppMessage) -> AppCommandResult {
        todo!()
    }

    async fn watch(&self, watch: Watch) {
        todo!()
    }
}