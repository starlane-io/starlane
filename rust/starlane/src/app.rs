use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use serde::{Deserialize, Serialize, Serializer};
use tokio::sync::{mpsc, Mutex, oneshot};
use tokio::time::Duration;

use crate::actor::{Actor, ActorArchetype, ActorAssign, ActorContext, ActorKey, ActorKind, ActorMeta, ActorRef, ActorSelect, MakeMeAnActor, NewActor};
use crate::actor;
use crate::artifact::{Artifact, ArtifactKey, Name};
use crate::core::{AppLaunchError, StarCoreCommand};
use crate::core::server::{AppExt, ActorCreateError};
use crate::error::Error;
use crate::filesystem::File;
use crate::frame::{ActorMessage, AppMessage};
use crate::id::{Id, IdSeq};
use crate::keys::{AppKey, SubSpaceKey, UserKey};
use crate::label::{Labels, LabelSelectionCriteria};
use crate::space::CreateAppControllerFail;
use crate::star::{ActorCreate, CoreAppSequenceRequest, CoreRequest, StarCommand, StarKey, StarVariantCommand, StarSkel};
use std::str::FromStr;


pub type AppKind = Name;


#[derive(Clone,Serialize,Deserialize)]
pub enum ConfigSrc
{
    None,
    Artifact(Artifact)
}

#[derive(Clone,Serialize,Deserialize)]
pub enum InitData
{
    None,
    Artifact(Artifact),
    Raw(Arc<Vec<u8>>),
    File(File)
}


pub struct AppSlice
{
    inner: Arc<Mutex<AppSliceInner>>
}

impl AppSlice
{
    pub fn new(assign: AppMeta, skel: StarSkel, ext: Arc<dyn AppExt>) -> Self
    {
        AppSlice {
            inner: Arc::new( Mutex::new(AppSliceInner {
                meta: assign,
                actors: HashMap::new(),
                sequence: Option::None,
                skel: skel,
                ext: ext
            }))
        }
    }

    pub fn context(&self)->AppContext
    {
        AppContext::new(self.inner.clone() )
    }

    pub async fn ext(&self)->Arc<AppExt>
    {
        let lock = self.inner.lock().await;
        lock.ext.clone()
    }

}

impl AppSlice
{
    async fn unique_seq(&self,user: UserKey)-> oneshot::Receiver<Arc<IdSeq>>
    {
        let lock = self.inner.lock().await;
        lock.unique_seq(user).await
    }

    async fn seq(&mut self)->Result<Arc<IdSeq>,Error>
    {
        let mut lock = self.inner.lock().await;
        (*lock).seq().await
    }


    async fn next_id(&mut self)->Result<Id,Error>
    {
        let mut lock = self.inner.lock().await;
        (*lock).next_id().await
    }

    async fn actor_create(&mut self, archetype: ActorArchetype) ->Result<ActorRef,ActorCreateError> {
        let context = AppContext::new( self.inner.clone() );
        let mut lock = self.inner.lock().await;
        (*lock).actor_create(context, archetype).await
    }

    async fn meta(&self) -> AppMeta {
        let lock = self.inner.lock().await;
        lock.meta.clone()
    }
}

/**
  * represents part of an app on one Server or Client star
  */
pub struct AppSliceInner
{
    pub meta: AppMeta,
    pub actors: HashMap<ActorKey,ActorRef>,
    pub sequence: Option<Arc<IdSeq>>,
    pub skel: StarSkel,
    pub ext: Arc<dyn AppExt>
}

impl AppSliceInner
{
    pub async fn unique_seq(&self,user: UserKey)-> oneshot::Receiver<Arc<IdSeq>>
    {
        let (tx,rx) = oneshot::channel();
        self.skel.manager_tx.send(StarVariantCommand::CoreRequest( CoreRequest::AppSequenceRequest(CoreAppSequenceRequest{
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

    pub async fn actor_create(&mut self, context: AppContext, archetype: ActorArchetype) ->Result<ActorRef,ActorCreateError>
    {
        let actor_id = self.next_id().await;
        if actor_id.is_err()
        {
            return Err(ActorCreateError::Error(actor_id.err().unwrap().to_string()));
        }

        let actor_id = actor_id.unwrap();

        let key = ActorKey{
            app: self.meta.key.clone(),
            id: actor_id
        };

        let meta = ActorMeta::new(key.clone(), archetype.kind.clone(), archetype.config.clone() );

        let mut context = ActorContext::new( meta, context );

        let actor = self.ext.actor_create( &mut context, archetype.clone() ).await?;

        let actor_ref = ActorRef{
            key: key.clone(),
            kind: archetype.kind.clone(),
            actor: actor
        };

        self.actors.insert( key.clone(), actor_ref.clone() );

        Ok( actor_ref )

    }

    pub fn meta(&self) -> AppMeta {
       self.meta.clone()
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
    pub config: ConfigSrc,
    pub owner: UserKey
}

impl AppMeta
{
    pub fn new(app: AppKey, kind: AppKind, config: ConfigSrc, owner:UserKey) -> Self
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


// this is everything describes what an App should be minus it's instance data (instance data like AppKey)
#[derive(Clone,Serialize,Deserialize)]
pub struct AppArchetype
{
    pub owner: UserKey,
    pub sub_space: SubSpaceKey,
    pub kind: AppKind,
    pub config: ConfigSrc,
    pub init: InitData,
    pub labels: Labels
}




#[derive(Clone,Serialize,Deserialize,Eq,PartialEq)]
pub enum AppStatus
{
    Unknown,
    Pending,
    Launching,
    Ready,
    Suspended,
    Resuming,
    Panic,
    Halting,
    Exited
}

impl FromStr for AppStatus{

    type Err = ();

    fn from_str(input: &str) -> Result<AppStatus, Self::Err> {
        match input {
            "Unknown"  => Ok(AppStatus::Unknown),
            "Pending"  => Ok(AppStatus::Pending),
            "Launching"  => Ok(AppStatus::Launching),
            "Ready"  => Ok(AppStatus::Ready),
            "Suspended"  => Ok(AppStatus::Suspended),
            "Resuming"  => Ok(AppStatus::Resuming),
            "Panic"  => Ok(AppStatus::Panic),
            "Halting"  => Ok(AppStatus::Halting),
            "Exited"  => Ok(AppStatus::Exited),
            _      => Err(()),
        }
    }
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
            AppStatus::Ready => "Ready".to_string(),
            AppStatus::Suspended => "Suspended".to_string(),
            AppStatus::Resuming => "Resuming".to_string(),
            AppStatus::Panic => "Panic".to_string(),
            AppStatus::Halting => "Halting".to_string(),
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

impl AppContext
{
    pub fn new( app: Arc<Mutex<AppSliceInner>> )->AppContext
    {
        AppContext{
            app: app
        }
    }

    pub async fn meta(&mut self)->AppMeta {
        let app = self.app.lock().await;
        app.meta().clone()
    }

    pub async fn unique_seq(&self,user: UserKey)-> oneshot::Receiver<Arc<IdSeq>> {
        let app = self.app.lock().await;
        app.unique_seq(user).await
    }

    pub async fn seq(&mut self)->Result<Arc<IdSeq>,Error> {
        let mut app = self.app.lock().await;
        app.seq().await
    }

    pub async fn next_id(&mut self)->Result<Id,Error>
    {
        let mut app = self.app.lock().await;
        app.next_id().await
    }

    pub async fn actor_create( &mut self , archetype: ActorArchetype) -> Result<ActorRef,ActorCreateError>
    {
        let context = AppContext::new(self.app.clone());
        let mut app = self.app.lock().await;
        app.actor_create(context, archetype).await
    }
}


pub struct AppContext
{
    app: Arc<Mutex<AppSliceInner>>
}
