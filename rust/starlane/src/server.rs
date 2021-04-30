use crate::frame::{ActorMessage, ActorEvent};
use tokio::sync::watch::{Sender, Receiver};

pub enum ServerMessageIn
{
   ResourceMessage(ActorMessage)
}

pub enum ServerMessageOut
{
    ResourceEvent(ActorEvent),
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