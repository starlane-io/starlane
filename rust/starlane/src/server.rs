use crate::frame::{ResourceMessage, EntityEvent};
use tokio::sync::watch::{Sender, Receiver};

pub enum ServerMessageIn
{
   ResourceMessage(ResourceMessage)
}

pub enum ServerMessageOut
{
    ResourceEvent(EntityEvent),
   ResourceMessage(ResourceMessage),
}

pub struct Server
{
    pub tx: Sender<ServerMessageOut>,
    pub rx: Receiver<ServerMessageIn>
}

impl Server
{

}