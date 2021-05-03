use crate::id::Id;
use tokio::sync::mpsc::{Sender};
use tokio::sync::broadcast::{Receiver};
use crate::star::StarKey;
use tokio::sync::{oneshot, mpsc};
use crate::actor::{ActorKey, ActorLocation, ActorKind};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::error::Error;
use crate::label::LabelSelectionCriteria;
use std::collections::HashMap;

pub type AppKey = Id;
pub type AppKind = String;

#[derive(Clone,Serialize,Deserialize)]
pub enum AppAccessCommand
{
    Create(AppCreate),
    Select(AppSelect),
    Destroy
}

#[derive(Clone,Serialize,Deserialize)]
pub enum AppCommand
{
    ActorCreate(ActorCreate),
    ActorSelect(ActorSelect),
    ActorDestroy(ActorKey)
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ActorCreate
{
    pub app: AppKey,
    pub kind: ActorKind,
    pub data: Arc<Vec<u8>>,
    pub labels: HashMap<String,String>
}

#[derive(Clone,Serialize,Deserialize)]
pub struct AppSelect
{
    criteria: Vec<LabelSelectionCriteria>
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ActorSelect
{
    criteria: Vec<LabelSelectionCriteria>
}

#[derive(Clone,Serialize,Deserialize)]
pub struct AppCreate
{
    pub name: Option<String>,
    pub kind: AppKind,
    pub data: Vec<u8>,
    pub labels: HashMap<String,String>
}


#[derive(Clone,Serialize,Deserialize)]
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

#[derive(Clone,Serialize,Deserialize)]
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

#[async_trait]
pub trait AppKindSupervisorExt
{
    async fn create( create: AppCreate ) -> Result<(),Error>;
    async fn destroy() -> Result<(),Error>;
    async fn handle( command: AppCommand ) -> Result<(),Error>;
}


