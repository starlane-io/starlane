use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::mpsc;

use crate::application::ApplicationState;
use crate::error::Error;
use crate::frame::{ResourceMessage, StarMessageInner, StarMessagePayload, StarUnwindPayload, StarWindInner, Watch, WatchInfo};
use crate::id::{Id, IdSeq};
use crate::resource::ResourceKey;
use crate::star::{StarCommand, StarKey, StarKind};
use std::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;


pub enum CoreCommand
{
    Message(ResourceMessage),
    Watch(Watch)
}

#[async_trait]
pub trait StarCoreFactory: Sync+Send
{
    async fn create( &self, kind: &StarKind ) -> mpsc::Sender<CoreCommand>;
}

pub struct StarCoreFactoryDefault
{
}

#[async_trait]
impl StarCoreFactory for StarCoreFactoryDefault
{
    async fn create(&self, kind: &StarKind) -> Sender<CoreCommand> {
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
    command_rx: mpsc::Receiver<CoreCommand>
}

impl ServerStarCore
{

    async fn run(&mut self)
    {
        while let Option::Some(command) = self.command_rx.recv().await
        {
            //process command
        }
    }
}