use crate::id::Id;
use tokio::sync::mpsc::{Sender};
use tokio::sync::broadcast::{Receiver};

pub enum AppCommand
{
}

pub enum AppEvent
{

}

pub struct Application
{
    pub app_id: Id,
    pub tx: Sender<AppCommand>,
    pub rx: Receiver<AppEvent>
}

pub enum ApplicationState
{
    None,
    Launching,
    Ready
}