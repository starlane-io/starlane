use std::collections::HashMap;
use std::sync::Arc;

use crate::actor::{Actor, ActorCreate, ActorKey, ActorKind, ActorSelect};
use crate::error::Error;
use crate::frame::{ActorMessage, AppCreate, AppMessage};
use crate::label::{Labels, LabelSelectionCriteria};
use crate::star::{StarCommand, StarKey};
use crate::keys::{AppKey, UserKey, SubSpaceKey};
use serde::{Deserialize, Serialize, Serializer};
use crate::space::{CreateAppControllerFail };
use tokio::sync::{oneshot, mpsc};
use std::fmt;


pub mod system;

pub type AppKind = String;

/**
  * represents part of an app on one Server or Client star
  */
pub struct AppSlice
{
    pub actors: HashMap<ActorKey,Box<dyn Actor>>
}


#[derive(Clone,Serialize,Deserialize)]
pub enum AppCommandKind
{
    AppMessage(AppMessage),
    Suspend,
    Resume,
    Exit
}

pub type AppMessageKind = String;


#[derive(Clone,Serialize,Deserialize)]
pub struct AppSelect
{
    criteria: Vec<LabelSelectionCriteria>
}

pub struct AppCreateController
{
    pub info: AppCreateData,
    pub tx: oneshot::Sender<Result<AppController,CreateAppControllerFail>>
}

#[derive(Clone,Serialize,Deserialize)]
pub struct AppDestroy
{

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
pub struct AppLocation
{
    pub app: AppKey,
    pub supervisor: StarKey
}

#[derive(Clone)]
pub struct AppController
{
    pub app: AppKey,
    pub tx: mpsc::Sender<AppMessage>
}

impl AppController
{

}

pub type Apps = HashMap<AppKind,Box<dyn Application>>;

pub struct AppContext
{
//    pub star_tx: mpsc::Sender<AppCommandWrapper>,
    pub info: AppInfo
}

#[derive(Clone,Serialize,Deserialize)]
pub struct AppCreateData
{
    pub owner: UserKey,
    pub sub_space: SubSpaceKey,
    pub kind: AppKind,
    pub data: Arc<Vec<u8>>,
    pub labels: Labels
}

#[async_trait]
pub trait Application: Send+Sync
{
    async fn create(&self, context: &AppContext, create: AppCreateController) -> Result<Labels,Error>;
    async fn destroy( &self, context: &AppContext, destroy: AppDestroy ) -> Result<(),Error>;
    async fn handle_app_command(&self, context: &AppContext, command: AppMessage) -> Result<(),Error>;
    async fn handle_actor_message( &self, context: &AppContext, actor: &mut Actor, message: ActorMessage  ) -> Result<(),Error>;
}


#[derive(Clone,Serialize,Deserialize)]
pub enum AppStatus
{
    Unknown,
    Waiting,
    Launching,
    Ready(AppReadyStatus),
    Suspended,
    Resuming,
    Panic(AppPanicReason),
    Halting(HaltReason),
    Exited
}


impl fmt::Display for AppStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = match self {
            AppStatus::Unknown => "Unknown".to_string(),
            AppStatus::Waiting => "Waiting".to_string(),
            AppStatus::Launching => "Launching".to_string(),
            AppStatus::Ready(_) => "Ready".to_string(),
            AppStatus::Suspended => "Suspended".to_string(),
            AppStatus::Resuming => "Resuming".to_string(),
            AppStatus::Panic(_) => "Panic".to_string(),
            AppStatus::Halting(_) => "Halting".to_string(),
            AppStatus::Exited => "Exited".to_string()
        };
        write!(f, "{}",r)
    }
}

#[derive(Clone,Serialize,Deserialize)]
pub enum HaltReason
{
    Planned,
    Crashing
}

#[derive(Clone,Serialize,Deserialize)]
pub enum AppReadyStatus
{
    Nominal,
    Alert(Alert)
}

#[derive(Clone,Serialize,Deserialize)]
pub enum Alert
{
    Red(AppAlertReason),
    Yellow(AppAlertReason)
}

pub type AppAlertReason = String;

#[derive(Clone,Serialize,Deserialize)]
pub enum AppPanicReason
{
    Desc(String)
}

#[derive(Clone,Serialize,Deserialize)]
pub enum AppCreateResult
{
    Ok,
    CannotCreateAppOfKind(AppKind),
    Error(String)
}

#[derive(Clone,Serialize,Deserialize)]
pub enum AppMessageResult
{
    Ok,
    Error(String)
}

#[derive(Clone,Serialize,Deserialize)]
pub enum ActorMessageResult
{
    Ok,
    Error(String)
}

