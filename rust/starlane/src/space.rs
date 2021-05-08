use crate::app::{AppCreateController, AppSelect, AppController, AppKind, AppCreateData};
use crate::keys::{SpaceKey, UserKey, AppKey, SubSpaceKey};
use serde::{Deserialize, Serialize, Serializer};
use std::fmt;
use tokio::sync::{mpsc, oneshot};
use std::sync::Arc;
use crate::permissions::Authentication;
use crate::error::Error;
use crate::label::Labels;

pub struct SpaceCommand
{
    pub space: SpaceKey,
    pub user: UserKey,
    pub kind: SpaceCommandKind
}

pub enum SpaceCommandKind
{
    AppCreateController(AppCreateController),
    AppGetController(AppSelect)
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

   pub async fn create_app( &self, kind: &AppKind, sub_space: &SubSpaceKey, data: &Arc<Vec<u8>>, labels: &Labels ) -> oneshot::Receiver<Result<AppController,CreateAppControllerFail>>
   {
       let (tx,rx) = oneshot::channel();

       let create = AppCreateData {
           owner: self.user.clone(),
           sub_space: sub_space.clone(),
           kind: kind.clone(),
           data: data.clone(),
           labels: labels.clone(),
       };

       let create_ctrl = AppCreateController
       {
           info: create,
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
}


impl fmt::Display for SpaceCommandKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = match self {
            SpaceCommandKind::AppCreateController(_) => "AppCreate".to_string(),
            SpaceCommandKind::AppGetController(_) => "AppSelect".to_string(),
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