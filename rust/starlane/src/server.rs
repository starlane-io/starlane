use crate::frame::{EntityMessage, EntityEvent};
use tokio::sync::watch::{Sender, Receiver};

pub enum ServerMessageIn
{
   ResourceMessage(EntityMessage)
}

pub enum ServerMessageOut
{
    ResourceEvent(EntityEvent),
   ResourceMessage(EntityMessage),
}

pub struct Server
{
    pub tx: Sender<ServerMessageOut>,
    pub rx: Receiver<ServerMessageIn>
}

impl Server
{

}