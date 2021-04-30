use crate::id::Id;
use tokio::sync::mpsc::{Sender};
use tokio::sync::broadcast::{Receiver};
use crate::star::StarKey;
use tokio::sync::{oneshot, mpsc};
use crate::actor::{ActorKey, ActorLocation};
use serde::{Deserialize, Serialize};
use std::sync::Arc;


pub type AppKey = Id;
pub type AppKind = String;

pub enum AppAccessCommand
{
    Create(AppCreate),
    Get(AppGet)
}

pub enum AppCommand
{
    ActorCreate(EntityCreate),
    Destroy
}

pub struct EntityCreate
{
    app: AppKey,
    data: Arc<Vec<u8>>,
    pub tx: oneshot::Sender<ActorKey>
}


#[derive(Clone)]
pub struct AppCreate
{
    pub name: Option<String>,
    pub kind: AppKind,
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

pub enum ApplicationStatus
{
    None,
    Launching,
    Ready
}

#[derive(Clone,Serialize,Deserialize)]
pub struct AppInfo
{
    pub key: AppKey,
    pub kind: AppKind
}

impl AppInfo
{
    pub fn new( key: AppKey, kind: AppKind ) -> Self
    {
        AppInfo
        {
            key: key,
            kind: kind
        }
    }
}

pub struct Application
{
    pub info: AppInfo,
    pub data: Vec<u8>
}

impl Application
{
    pub fn new( info: AppInfo, data: Vec<u8> ) -> Self
    {
        Application
        {
            info: info,
            data: data
        }
    }
}


#[derive(Clone,Serialize,Deserialize)]
pub struct AppLocation
{
    pub app: AppKey,
    pub supervisor: StarKey
}

#[derive(Clone)]
pub struct AppController
{
    pub app: AppKey,
    pub tx: Sender<AppCommand>
}