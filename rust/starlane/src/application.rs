use crate::id::Id;
use tokio::sync::mpsc::{Sender};
use tokio::sync::broadcast::{Receiver};
use crate::star::StarKey;
use tokio::sync::{oneshot, mpsc};
use crate::entity::{EntityKey, EntityLocation};
use serde::{Deserialize, Serialize};

pub enum AppLifecycleCommand
{
    Create(AppCreate),
    Get(AppGet),
    Destroy(Id)
}

pub enum AppCommand
{
    ResourceCreate(ResourceCreate)
}

pub struct ResourceCreate
{
    app_id: Id,
    data: Vec<u8>,
    pub tx: oneshot::Sender<EntityKey>
}


#[derive(Clone)]
pub struct AppCreate
{
    pub name: Option<String>,
    pub data: Vec<u8>,
    pub tx: mpsc::Sender<AppController>,
}

#[derive(Clone)]
pub struct AppGet
{
    pub tx: mpsc::Sender<AppController>,
    pub lookup: AppLookup
}

#[derive(Clone)]
pub enum AppLookup
{
    Name(String),
    Id(Id)
}

pub enum AppEvent
{

}

pub struct Application
{
    pub app_id: Id,
    pub tx: Sender<AppLifecycleCommand>,
    pub rx: Receiver<AppEvent>
}

pub enum ApplicationState
{
    None,
    Launching,
    Ready
}

#[derive(Clone,Serialize,Deserialize)]
pub struct AppLocation
{
    pub app_id: Id,
    pub supervisor: StarKey
}

#[derive(Clone)]
pub struct AppController
{
    pub app_id: Id,
    pub tx: Sender<AppCommand>
}