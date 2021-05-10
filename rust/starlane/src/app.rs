use std::collections::HashMap;
use std::sync::Arc;

use crate::actor::{Actor, MakeMeAnActor, ActorKey, ActorKind, ActorSelect, ActorRef, NewActor, ActorAssign};
use crate::error::Error;
use crate::frame::{ActorMessage, AppCreate, AppMessage};
use crate::label::{Labels, LabelSelectionCriteria};
use crate::star::{StarCommand, StarKey, StarSkel, StarManagerCommand, CoreRequest, CoreAppSequenceRequest, ActorCreate};
use crate::keys::{AppKey, UserKey, SubSpaceKey};
use serde::{Deserialize, Serialize, Serializer};
use crate::space::{CreateAppControllerFail };
use tokio::sync::{oneshot, mpsc};
use std::fmt;
use crate::id::{IdSeq, Id};
use crate::core::StarCoreCommand;
use crate::frame::RequestMessage::AppSequenceRequest;
use tokio::time::Duration;
use crate::core::server::AppExt;
use crate::actor;

pub mod system;

pub type AppKind = String;


pub struct AppArchetype
{

}



/**
  * represents part of an app on one Server or Client star
  */
pub struct AppSlice
{
    pub info: AppInfo,
    pub owner: UserKey,
    pub actors: HashMap<ActorKey,Arc<ActorRef>>,
    sequence: Option<Arc<IdSeq>>,
    skel: StarSkel,
    ext: Box<dyn AppExt>
}

impl AppSlice
{
    pub fn new( info: AppInfo, owner: UserKey, skel: StarSkel, ext: Box<dyn AppExt> )->Self
    {
        AppSlice{
            info: info,
            owner: owner,
            actors: HashMap::new(),
            sequence: Option::None,
            skel: skel,
            ext: ext
        }
    }

    pub async fn unique_seq(&self,user: UserKey)-> oneshot::Receiver<Arc<IdSeq>>
    {
        let (tx,rx) = oneshot::channel();
        self.skel.manager_tx.send(StarManagerCommand::CoreRequest( CoreRequest::AppSequenceRequest(CoreAppSequenceRequest{
            app: self.info.key.clone(),
            user: user.clone(),
            tx: tx
        }) )).await;

        let (seq_tx, seq_rx) = oneshot::channel();

        tokio::spawn( async move {
            if let Result::Ok(Result::Ok(seq)) = tokio::time::timeout(Duration::from_secs(15), rx).await
            {
                seq_tx.send( Arc::new( IdSeq::new(seq)));
            }
        });

        seq_rx
    }

    pub async fn seq(&mut self)->Result<Arc<IdSeq>,Error>
    {
        if let Option::None = self.sequence
        {
            let rx = self.unique_seq(self.owner.clone()).await;
            let seq = rx.await?;
            self.sequence = Option::Some(seq);
        }

        Ok(self.sequence.as_ref().unwrap().clone())
    }


    pub async fn next_id(&mut self)->Result<Id,Error>
    {
        Ok(self.seq().await?.next())
    }

    pub async fn actor_create(&mut self, create: actor::MakeMeAnActor) ->Result<Arc<ActorRef>,Error>
    {
        unimplemented!()
        /*
        let key = ActorKey{
            app: self.app.clone(),
            id: self.next_id()?
        };

        let kind  = create.kind.clone();

        let assign = ActorAssign{
            key: key,
            kind: create.kind,
            data: create.data,
            labels: create.labels
        };

        let actor= self.ext.actor_create(self, assign).await?;

        let actor_ref = Arc::new(ActorRef{
            key: key.clone(),
            kind: kind.clone(),
            actor: actor
        });

        self.actors.insert( key.clone(), actor_ref.clone() );

        Ok( actor_ref )

         */
    }

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

