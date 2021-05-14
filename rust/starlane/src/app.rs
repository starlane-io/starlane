use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

use serde::{Deserialize, Serialize, Serializer};
use tokio::sync::{mpsc, Mutex, oneshot};
use tokio::time::Duration;

use crate::actor::{Actor, ActorArchetype, ActorAssign, ActorContext, ActorKey, ActorKind, ActorMeta, ActorRegistration, ActorMessage, MessageFrom, ActorKeySeqListener};
use crate::actor;
use crate::artifact::{Artifact, ArtifactKey};
use crate::core::{StarCoreCommand };
use crate::core::server::{AppExt};
use crate::error::Error;
use crate::filesystem::File;
use crate::frame::{Reply};
use crate::id::{Id, IdSeq};
use crate::keys::{AppKey, SubSpaceKey, UserKey, ResourceKey};
use crate::resource::{Labels, Resource, ResourceKind, ResourceRegistration};
use crate::names::Name;
use crate::space::CreateAppControllerFail;
use crate::star::{ActorCreate, CoreAppSequenceRequest, CoreRequest, StarCommand, StarKey, StarSkel, StarVariantCommand, StarComm, ServerCommand, Request, Empty};
use crate::message::Fail;
use tokio::sync::mpsc::Sender;
use tokio::time::error::Elapsed;
use tokio::sync::oneshot::error::RecvError;

pub type AppSpecific = Name;

#[derive(Clone,Serialize,Deserialize,Hash,Eq,PartialEq)]
pub enum AppKind
{
    Normal,
}


impl fmt::Display for AppKind{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!( f,"{}",
                match self{
                    AppKind::Normal => "Normal".to_string()
                })
    }
}


impl FromStr for AppKind
{
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s
        {
            "Normal" => Ok(AppKind::Normal),
            _ => Err(format!("could not find AppKind: {}",s).into())
        }
    }
}


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
    Memory(Memory),
    File(File)
}

#[derive(Clone,Serialize,Deserialize)]
pub struct Memory
{
    data: Arc<Vec<u8>>
}

impl Memory
{
    pub fn new(data: Vec<u8>)->Result<Self,Error>
    {
       if data.len() > 32*1024
       {
           Err(format!("in memory data limit is {}",(32*1024)).into())
       }
       else {
           Ok(Memory{
               data: Arc::new(data)
           })
       }
    }
}



pub enum AppSliceCommand {
    FetchSequence(Request<Empty,u64>),
    ClaimActor(ActorKey),
    Launch(Request<AppArchetype,()>)
}

/**
  * represents part of an app on one Server or Client star
  */
pub struct AppSlice
{
    pub meta: AppMeta,
    pub comm: StarComm,
    pub ext: Box<dyn AppExt>,
    pub rx: mpsc::Receiver<AppSliceCommand>,
    pub context: AppContext
}

impl AppSlice
{
    pub async fn new( meta: AppMeta, comm: StarComm, ext: Box<dyn AppExt>) -> mpsc::Sender<AppSliceCommand>
    {
        let (tx,rx) = mpsc::channel(1024);

        let context = AppContext::new(meta.clone(), tx.clone(), comm.clone() );
        let app = AppSlice{
            meta: meta,
            context: context,
            comm: comm,
            ext: ext,
            rx: rx
        };

        tokio::spawn(async move { app.run().await; } );

        tx
    }

    async fn run(mut self)
    {
        while let Option::Some(command) = self.rx.recv().await {
            self.process(command).await;
        }
    }

    async fn process( &mut self, command: AppSliceCommand )->Result<(),Error>
    {
        match command
        {
            AppSliceCommand::FetchSequence(request) => {
                self.fetch_seq(request).await;
                Ok(())
            }
            AppSliceCommand::ClaimActor(claim) => {
                // not sure what to do with this yet...
                Ok(())
            }
            AppSliceCommand::Launch(request) => {
                let result = self.ext.launch(request.payload ).await;
                request.tx.send(result);
                Ok(())
            }
        }
    }

    async fn fetch_seq(&mut self, request: Request<Empty,u64>) {
        let (tx,rx) = oneshot::channel();
        self.comm.variant_tx.send( StarVariantCommand::CoreRequest(CoreRequest::AppSequenceRequest(CoreAppSequenceRequest{
            app: self.meta.key.clone(),
            user: self.meta.owner.clone(),
            tx: tx
        }))).await;
        tokio::spawn( async move {
            match tokio::time::timeout( Duration::from_secs(10),rx).await
            {
                Ok(result) => {
                    match result
                    {
                        Ok(seq) => {
                            request.tx.send(Result::Ok(seq));
                        }
                        Err(err) => {
                            request.tx.send(Result::Err(Fail::Unexpected));
                        }
                    }
                }
                Err(err) => {
                    request.tx.send(Result::Err(Fail::Timeout));
                }
            }
        } );
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
    pub specific: AppSpecific,
    pub config: ConfigSrc,
    pub owner: UserKey
}

impl AppMeta
{
    pub fn new(app: AppKey, kind: AppKind, specific: AppSpecific, config: ConfigSrc, owner:UserKey) -> Self
    {
        AppMeta
        {
            key: app,
            kind: kind,
            specific: specific,
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
    pub fn new(key: AppKey, archetype: AppArchetype ) -> Self
    {
        App{
            key: key,
            archetype: archetype
        }
    }

    pub fn meta(&self)->AppMeta
    {
        AppMeta{
            key: self.key.clone(),
            kind: self.archetype.kind.clone(),
            specific: self.archetype.specific.clone(),
            config: self.archetype.config.clone(),
            owner: self.archetype.owner.clone()
        }
    }
}

impl From<App> for Resource
{
    fn from(app: App) -> Self {
        Resource{
            key: ResourceKey::App(app.key.clone()),
            kind: ResourceKind::App(app.archetype.kind),
            owner: Option::Some(app.archetype.owner),
            specific: Option::Some(app.archetype.specific)
        }
    }
}

#[derive(Clone,Serialize,Deserialize)]
pub struct AppLocation
{
    pub app: AppKey,
    pub supervisor: StarKey
}

#[derive(Clone,Serialize,Deserialize)]
pub enum AppCommand
{

}

#[derive(Clone)]
pub struct AppController
{
    pub app: AppKey,
    pub tx: mpsc::Sender<AppCommand>
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
    pub specific: AppSpecific,
    pub config: ConfigSrc,
    pub init: InitData,
    pub name: Option<String>,
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
    CannotCreateAppOfKind(AppSpecific),
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

#[derive(Clone)]
pub struct AppContext
{
    meta: AppMeta,
    sequence: Option<Arc<IdSeq>>,
    app_tx: mpsc::Sender<AppSliceCommand>,
    comm: StarComm
}

impl AppContext
{
    pub fn new( meta: AppMeta, app_tx: mpsc::Sender<AppSliceCommand>, comm: StarComm )->Self
    {
        AppContext{
            app_tx: app_tx,
            meta: meta,
            sequence: Option::None,
            comm: comm
        }
    }

    pub async fn meta(&mut self)->AppMeta {
        self.meta.clone()
    }

    pub async fn unique_seq(&mut self)->Result<Arc<IdSeq>,Fail>
    {
        let (request,rx) = Request::new(Empty::new() );
        self.app_tx.send( AppSliceCommand::FetchSequence(request)).await;
        if let seq_id= rx.await??
        {
            Ok(Arc::new(IdSeq::new(seq_id)))
        }
        else
        {
            Err(Fail::Unexpected)
        }

    }

    pub async fn seq(&mut self)->Result<Arc<IdSeq>,Fail>
    {
        if let Option::None = self.sequence
        {
            self.sequence = Option::Some(self.unique_seq().await?)
        }

        Ok(self.sequence.as_ref().unwrap().clone())
    }


    pub async fn next_id(&mut self)->Result<Id,Fail>
    {
        Ok(self.seq().await?.next())
    }

    pub async fn create_actor_key(&mut self) ->Result<ActorKey,Fail>
    {
        let actor_id = self.next_id().await?;

        let actor_key = ActorKey{
            app: self.meta.key.clone(),
            id: actor_id
        };

        Ok( actor_key )
    }

    pub async fn register(&mut self, registration: ActorRegistration ) -> Result<(),Fail>
    {
        let registration: ResourceRegistration = registration.into();
        let (request,rx) = Request::new(registration);
        self.comm.variant_tx.send( StarVariantCommand::ServerCommand(ServerCommand::Register(request))).await;
        rx.await?;
        Ok(())
    }
}

#[derive(Clone,Serialize,Deserialize)]
pub struct AppTo{
    pub app: AppKey,
    pub ext: Option<Raw>
}

#[derive(Clone,Serialize,Deserialize)]
pub struct AppMessage
{
    pub to: AppTo,
    pub from: MessageFrom,
    pub payload: Arc<Raw>
}

#[derive(Clone,Serialize,Deserialize)]
pub struct AppFrom{
    pub app: AppKey,
    pub ext: Option<Raw>
}

pub type Raw=Vec<u8>;
