use crate::app::{AppCreateController, AppController, AppSpecific, AppArchetype, InitData, ConfigSrc, AppKind, AppProfile};
use crate::keys::{SpaceKey, UserKey, AppKey, SubSpaceKey};
use serde::{Deserialize, Serialize, Serializer};
use std::fmt;
use tokio::sync::{mpsc, oneshot};
use std::sync::Arc;
use crate::permissions::Authentication;
use crate::error::Error;
use crate::resource::{Labels, Selector};
use crate::artifact::Artifact;
use crate::names::Name;
use crate::message::Fail;

pub struct RemoteSpaceCommand
{
    pub space: SpaceKey,
    pub user: UserKey,
    pub kind: RemoteSpaceCommandKind
}

pub enum RemoteSpaceCommandKind
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
    tx: mpsc::Sender<RemoteSpaceCommand>
}

impl SpaceController
{
   pub fn new(user: UserKey, tx: mpsc::Sender<RemoteSpaceCommand> ) -> Self
   {
       SpaceController{
           user: user,
           tx: tx
       }
   }

   pub async fn create_app(&self, kind: &AppKind, specific: &AppSpecific, config: &ConfigSrc, init: &InitData, sub_space: &SubSpaceKey, name: Option<String>, labels: &Labels ) -> oneshot::Receiver<Result<AppController,CreateAppControllerFail>>
   {
       unimplemented!()
       /*
       let (tx,rx) = oneshot::channel();

       let profile = AppProfile{
           init: InitData::None,
           archetype: AppArchetype {
               kind: kind.clone(),
               specific: specific.clone(),
               config: config.clone()
           },
       };

       let create_ctrl = AppCreateController
       {
           sub_space: sub_space.clone(),
           profile,
           tx: tx
       };

       let command = RemoteSpaceCommand {
           space: sub_space.space.clone(),
           user: self.user.clone(),
           kind: RemoteSpaceCommandKind::AppCreateController(create_ctrl)
       };

       self.tx.send( command ).await;

       rx
        */
   }

   pub async fn select_apps(&self, selector: Selector, sub_space: SubSpaceKey ) -> oneshot::Receiver<Result<Vec<AppKey>,Fail>>
   {
       let (tx,rx) = oneshot::channel();

       let command = RemoteSpaceCommand {
           space: sub_space.space.clone(),
           user: self.user.clone(),
           kind: RemoteSpaceCommandKind::AppSelect(AppSelectCommand{

               selector,
               tx
           })
       };

       self.tx.send( command ).await;

       rx
   }


}


impl fmt::Display for RemoteSpaceCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = match self {
            RemoteSpaceCommand::AppCreateController(_) => "AppCreate".to_string(),
            RemoteSpaceCommand::AppSelect(_) => "AppSelect".to_string()
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