use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::mpsc;

use crate::application::ApplicationStatus;
use crate::error::Error;
use crate::frame::{ResourceMessage, StarMessageInner, StarMessagePayload, StarUnwindPayload, StarWindInner, Watch, WatchInfo};
use crate::id::{Id, IdSeq};
use crate::entity::EntityKey;
use crate::star::{StarCommand, StarKey, StarKind, EntityCommand};
use std::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;


pub enum StarCoreCommand
{
    Message(ResourceMessage),
    Watch(Watch),
    Entity(EntityCommand)
}

#[async_trait]
pub trait StarCoreFactory: Sync+Send
{
    async fn create( &self, kind: &StarKind ) -> mpsc::Sender<StarCoreCommand>;
}

pub struct StarCoreFactoryDefault
{
}

#[async_trait]
impl StarCoreFactory for StarCoreFactoryDefault
{
    async fn create(&self, kind: &StarKind) -> Sender<StarCoreCommand> {
       let (tx,rx) = mpsc::channel(32);
       let mut core = ServerStarCore{
           command_rx: rx
       };

       tokio::spawn( async move {
           core.run().await
       } );

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