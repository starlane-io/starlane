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


pub enum CoreCommand
{
    Message(ResourceMessage),
    Watch(Watch)
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