use crate::frame::{ActorMessage, Event};
use tokio::sync::watch::{Sender, Receiver};

pub enum ServerMessageIn
{
   ResourceMessage(ActorMessage)
}

pub enum ServerMessageOut
{
    ResourceEvent(Event),
   ResourceMessage(ActorMessage),
}

pub struct Server
{
    pub tx: Sender<ServerMessageOut>,
    pub rx: Receiver<ServerMessageIn>
}

impl Server
{

}