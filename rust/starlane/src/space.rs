use crate::app::{AppCreateController,  AppController, AppSpecific, AppArchetype, InitData, ConfigSrc, AppKind};
use crate::keys::{SpaceKey, UserKey, AppKey, SubSpaceKey};
use serde::{Deserialize, Serialize, Serializer};
use std::fmt;
use tokio::sync::{mpsc, oneshot};
use std::sync::Arc;
use crate::permissions::Authentication;
use crate::error::Error;
use crate::label::{Labels, Selector};
use crate::artifact::Artifact;
use crate::names::Name;
use crate::message::Fail;

pub struct SpaceCommand
{
    pub space: SpaceKey,
    pub user: UserKey,
    pub kind: SpaceCommandKind
}

pub enum SpaceCommandKind
{
    AppCreateController(AppCreateController),
    AppSelect(AppSelectCommand)
}

pub struct AppSelectCommand
{
    pub selector: Selector,
    pub tx: oneshot::Sender<Result<Vec<AppKey>,Fail>>
}

pub struct SpaceController
{
    user: UserKey,
    tx: mpsc::Sender<SpaceCommand>
}

impl SpaceController
{
   pub fn new(user: UserKey, tx: mpsc::Sender<SpaceCommand> ) -> Self
   {
       SpaceController{
           user: user,
           tx: tx
       }
   }

   pub async fn create_app(&self, kind: &AppKind, specific: &AppSpecific, config: &ConfigSrc, init: &InitData, sub_space: &SubSpaceKey, name: Option<String>, labels: &Labels ) -> oneshot::Receiver<Result<AppController,CreateAppControllerFail>>
   {
       let (tx,rx) = oneshot::channel();

       let create = AppArchetype {
           owner: self.user.clone(),
           sub_space: sub_space.clone(),
           kind: kind.clone(),
           specific: specific.clone(),
           config: config.clone(),
           init: init.clone(),
           labels: labels.clone(),
           name: name
       };

       let create_ctrl = AppCreateController
       {
           archetype: create,
           tx: tx
       };

       let command = SpaceCommand{
           space: sub_space.space.clone(),
           user: self.user.clone(),
           kind: SpaceCommandKind::AppCreateController(create_ctrl)
       };

       self.tx.send( command ).await;

       rx
   }

   pub async fn select_apps(&self, selector: Selector, sub_space: SubSpaceKey ) -> oneshot::Receiver<Result<Vec<AppKey>,Fail>>
   {
       let (tx,rx) = oneshot::channel();

       let command = SpaceCommand{
           space: sub_space.space.clone(),
           user: self.user.clone(),
           kind: SpaceCommandKind::AppSelect(AppSelectCommand{
               selector,
               tx
           })
       };

       self.tx.send( command ).await;

       rx
   }


}


impl fmt::Display for SpaceCommandKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = match self {
            SpaceCommandKind::AppCreateController(_) => "AppCreate".to_string(),
            SpaceCommandKind::AppSelect(_) => "AppSelect".to_string()
        };
        write!(f, "{}",r)
    }
}


pub enum CreateAppControllerFail
{
    PermissionDenied,
    SpacesDoNotMatch,
    UnexpectedResponse,
    Timeout,
    Error(Error)
}