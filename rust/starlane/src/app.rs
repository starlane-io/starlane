use std::collections::HashMap;
use std::sync::Arc;

use crate::actor::{Actor, MakeMeAnActor, ActorKey, ActorKind, ActorSelect, ActorRef, NewActor, ActorAssign};
use crate::error::Error;
use crate::frame::{ActorMessage, AppMessage};
use crate::label::{Labels, LabelSelectionCriteria};
use crate::star::{StarCommand, StarKey, StarSkel, StarManagerCommand, CoreRequest, CoreAppSequenceRequest, ActorCreate};
use crate::keys::{AppKey, UserKey, SubSpaceKey};
use serde::{Deserialize, Serialize, Serializer};
use crate::space::{CreateAppControllerFail };
use tokio::sync::{oneshot, mpsc};
use std::fmt;
use crate::id::{IdSeq, Id};
use crate::core::StarCoreCommand;
use tokio::time::Duration;
use crate::core::server::AppExt;
use crate::actor;
use crate::artifact::{ArtifactKey, Artifact, Name};
use crate::filesystem::File;

pub mod system;

pub type AppKind = Name;


#[derive(Clone,Serialize,Deserialize)]
pub enum AppConfigSrc
{
    None,
    Artifact(Artifact)
}

#[derive(Clone,Serialize,Deserialize)]
pub enum AppInitData
{
    None,
    Artifact(Artifact),
    Raw(Arc<Vec<u8>>),
    File(File)
}



/**
  * represents part of an app on one Server or Client star
  */
pub struct AppSlice
{
    pub meta: AppMeta,
    pub actors: HashMap<ActorKey,Arc<ActorRef>>,
    sequence: Option<Arc<IdSeq>>,
    skel: StarSkel,
    ext: Box<dyn AppExt>
}

impl AppSlice
{
    pub fn new(assign: AppMeta, skel: StarSkel, ext: Box<dyn AppExt> ) ->Self
    {
        AppSlice{
            meta: assign,
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
            app: self.meta.key.clone(),
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
            let rx = self.unique_seq(self.meta.owner.clone()).await;
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
    pub archetype: AppArchetype,
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
pub struct AppMeta
{
    pub key: AppKey,
    pub kind: AppKind,
    pub config: AppConfigSrc,
    pub owner: UserKey
}

impl AppMeta
{
    pub fn new( app: AppKey, kind: AppKind, config: AppConfigSrc, owner:UserKey) -> Self
    {
        AppMeta
        {
            key: app,
            kind: kind,
            config: config,
            owner: owner
        }
    }
}

#[derive(Clone,Serialize,Deserialize)]
pub struct App
{
    pub key: AppKey,
    pub archetype: AppArchetype
}

impl App
{
    pub fn meta(&self)->AppMeta
    {
        AppMeta{
            key: self.key.clone(),
            kind: self.archetype.kind.clone(),
            config: self.archetype.config.clone(),
            owner: self.archetype.owner.clone()
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
    pub info: AppMeta
}

// this is everything describes what an App should be minus it's instance data (instance data like AppKey)
#[derive(Clone,Serialize,Deserialize)]
pub struct AppArchetype
{
    pub owner: UserKey,
    pub sub_space: SubSpaceKey,
    pub kind: AppKind,
    pub config: AppConfigSrc,
    pub init: AppInitData,
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


#[derive(Clone,Serialize,Deserialize,Eq,PartialEq)]
pub enum AppStatus
{
    Unknown,
    Pending,
    Launching,
    Ready(AppReadyStatus),
    Suspended,
    Resuming,
    Panic(AppPanicReason),
    Halting(HaltReason),
    Exited
}




#[derive(Clone,Serialize,Deserialize,Eq,PartialEq)]
pub enum HaltReason
{
    Planned,
    Crashing
}

#[derive(Clone,Serialize,Deserialize,Eq,PartialEq)]
pub enum AppReadyStatus
{
    Nominal,
    Alert(Alert)
}

#[derive(Clone,Serialize,Deserialize,Eq,PartialEq)]
pub enum Alert
{
    Red(AppAlertReason),
    Yellow(AppAlertReason)
}

pub type AppAlertReason = String;

#[derive(Clone,Serialize,Deserialize,Eq,PartialEq)]
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

impl fmt::Display for AppStatus{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = match self {
            AppStatus::Unknown => "Unknown".to_string(),
            AppStatus::Pending => "Pending".to_string(),
            AppStatus::Launching => "Launching".to_string(),
            AppStatus::Ready(status) => format!("Ready({})",status,).to_string(),
            AppStatus::Suspended => "Suspended".to_string(),
            AppStatus::Resuming => "Resuming".to_string(),
            AppStatus::Panic(_) => "Panic".to_string(),
            AppStatus::Halting(halting) => format!("Halting({})",halting).to_string(),
            AppStatus::Exited => "Unknown".to_string(),
        };
        write!(f, "{}",r)
    }
}

impl fmt::Display for AppReadyStatus{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = match self {
            AppReadyStatus::Nominal => "Nominal".to_string(),
            AppReadyStatus::Alert(alert) => format!("Alert({})",alert).to_string()
        };
        write!(f, "{}",r)
    }
}

impl fmt::Display for Alert{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = match self {
            Alert::Red(_) => "Red".to_string(),
            Alert::Yellow(_) => "Yellow".to_string()
        };
        write!(f, "{}",r)
    }
}

impl fmt::Display for HaltReason{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = match self {
            HaltReason::Planned => "Planned".to_string(),
            HaltReason::Crashing => "Crashing".to_string()
        };
        write!(f, "{}",r)
    }
}
