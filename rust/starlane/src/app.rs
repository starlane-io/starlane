use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

use serde::{Deserialize, Serialize, Serializer};
use tokio::sync::{mpsc, Mutex, oneshot};
use tokio::time::Duration;

use crate::actor::{Actor, ActorArchetype, ActorAssign, ActorContext, ActorKey, ActorKind, ActorMeta, ActorRegistration, ResourceMessage, ActorKeySeq, ActorStatus, RawPayload, ResourceTo, ResourceFrom, ActorResource};
use crate::actor;
use crate::artifact::{Artifact, ArtifactKey};
use crate::core::{StarCoreCommand };
use crate::core::server::{AppExt};
use crate::error::Error;
use crate::filesystem::File;
use crate::frame::{Reply, StarMessagePayload, ResourceManagerAction};
use crate::id::{Id, IdSeq};
use crate::keys::{AppKey, SubSpaceKey, UserKey, ResourceKey};
use crate::resource::{Labels, ResourceAssign, ResourceKind, ResourceRegistration, ResourceLocation, ResourceArchetype, ResourceProfile, ResourceAddress, Names, ResourceSrc, Skewer, ResourceAddressPart, ResourceType, Resource};
use crate::names::Name;
use crate::space::CreateAppControllerFail;
use crate::star::{ActorCreate, CoreAppSequenceRequest, CoreRequest, StarCommand, StarKey, StarSkel, StarVariantCommand, StarComm, ServerCommand, Request, Empty, Query, LocalResourceLocation, ResourceCommand};
use crate::message::{Fail, ProtoMessage};
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
    Artifact(Artifact),
    ResourceAddressPart(ResourceAddressPart)
}

impl ToString for ConfigSrc {

    fn to_string(&self) -> String {
        match self
        {
            ConfigSrc::Artifact(artifact) => format!("Artifact::{}",artifact.to_string()),
            ConfigSrc::ResourceAddressPart(address) => format!("ResourceAddressPart::{}", address.to_string()),
        }
    }
}

impl FromStr for ConfigSrc {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut split = s.split("::");
        match split.next()?{
            "Artifact" => Ok(ConfigSrc::Artifact(Artifact::from_str(split.next()?))),
            "ResourceAddress" => Ok(ConfigSrc::ResourceAddressPart(ResourceAddress::from_str(split.next()?)?)),
            what => Err(format!("cannot process ConfigSrc of type {}",what).to_owned().into())
        }
    }
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
    FetchAddress(Request<ResourceKey,ResourceAddress>),
    Launch(Request<AppArchetype,()>),
    AddActor(ActorKey),
    HasActor(Request<ResourceKey, LocalResourceLocation>),
    AppMessage(ResourceMessage),
    ActorMessage(Request<ResourceMessage,()>),
}

/**
  * represents part of an app on one Server or Client star
  */
pub struct AppSlice
{
    pub comm: StarComm,
    pub ext: Box<dyn AppExt>,
    pub rx: mpsc::Receiver<AppSliceCommand>,
    pub context: AppContext,
    pub resource: AppResource,
    pub actors: HashMap<ActorKey,ActorStatus>
}

impl AppSlice
{
    pub async fn new( resource: AppResource, comm: StarComm, ext: Box<dyn AppExt>) -> mpsc::Sender<AppSliceCommand>
    {
        let (tx,rx) = mpsc::channel(1024);

        let context = AppContext::new(resource.clone(), tx.clone(), comm.clone() );
        let app = AppSlice{
            resource: resource,
            context: context,
            comm: comm,
            ext: ext,
            rx: rx,
            actors: HashMap::new()
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
            AppSliceCommand::Launch(request) => {
                let result = self.ext.launch(request.payload ).await;
                request.tx.send(result);
                Ok(())
            }
            AppSliceCommand::AddActor(actor) => {
                self.actors.insert(actor.clone(),ActorStatus::Unknown );
                let local = LocalResourceLocation::new(ResourceKey::Actor(actor), Option::None );
                self.comm.variant_tx.send(StarVariantCommand::ResourceCommand(ResourceCommand::SignalLocation(local))).await;
                Ok(())
            }
            AppSliceCommand::HasActor(request) => {
                if let ResourceKey::Actor(actor)=request.payload
                {
                    if self.actors.contains_key(&actor) {
                        let local = LocalResourceLocation::new(ResourceKey::Actor(actor), Option::None);
                        request.tx.send(Result::Ok(local));
                    } else {
                        request.tx.send(Result::Err(Fail::ResourceNotFound(ResourceKey::Actor(actor))));
                    }
                } else {
                    request.tx.send(Result::Err(Fail::ResourceNotFound(request.payload)));
                }
                Ok(())
            }
            AppSliceCommand::AppMessage(message) => {
                let result = self.ext.app_message(message).await;
                Ok(())
            }
            AppSliceCommand::ActorMessage(request) => {
                if let ResourceKey::Actor(key) = &request.payload.to.key
                {
                    if self.actors.contains_key(key) {
                        request.tx.send(Ok(()) );
                        let result = self.ext.actor_message(request.payload).await;
                    } else {
                        request.tx.send(Err(Fail::ResourceNotFound(request.payload.to.key)));
                    }
                } else {
                    request.tx.send(Err(Fail::WrongResourceType));
                }
                Ok(())
            }
        }
    }

    async fn fetch_seq(&mut self, request: Request<Empty,u64>) {
        let (tx,rx) = oneshot::channel();
        self.comm.variant_tx.send( StarVariantCommand::CoreRequest(CoreRequest::AppSequenceRequest(CoreAppSequenceRequest{
            app: self.resource.key.clone(),
            user: self.resource.owner.clone(),
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
    AppMessage(ResourceMessage),
    Suspend,
    Resume,
    Exit
}

pub type AppMessageKind = String;




pub struct AppCreateController
{
    pub sub_space: SubSpaceKey,
    pub profile: AppProfile,
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

#[derive(Clone,Serialize,Deserialize)]
pub struct AppProfile
{
    pub archetype: AppArchetype,
    pub init: InitData,
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ActorProfile{
    pub archetype: ActorArchetype,
    pub init: InitData
}

impl From<AppProfile> for ResourceProfile
{
    fn from(profile: AppProfile) -> Self {
        ResourceProfile {
            init: profile.init,
            archetype: profile.archetype.into(),
        }
    }
}

// this is everything describes what an App should be minus it's instance data (instance data like AppKey)
#[derive(Clone,Serialize,Deserialize)]
pub struct AppArchetype
{
    pub kind: AppKind,
    pub specific: AppSpecific,
    pub config: ConfigSrc,
}

impl From<AppArchetype> for ResourceArchetype
{
    fn from(archetype: AppArchetype) -> Self {
        ResourceArchetype{
            kind: ResourceKind::App(archetype.kind),
            specific: Option::Some(archetype.specific),
            config: Option::Some(archetype.config)
        }
    }
}




impl AppArchetype {
    pub fn resource_archetype(self)->ResourceArchetype{
        ResourceArchetype::App(self)
    }
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
pub struct AppResource{
    pub key: AppKey,
    pub archetype: AppArchetype,
    pub address: ResourceAddress,
    pub owner: UserKey
}

impl AppResource {
    pub fn from_resource( resource: Resource ) -> Result<AppResource,Error> {
        if !resource.validate(ResourceType::App) {
            return Err("resource is not completely an AppResource".into());
        }

        if let ResourceKind::App(kind) = resource.archetype.kind{
            let resource = AppResource{
                key: resource.key.app()?,
                archetype: AppArchetype{
                    kind: kind,
                    specific: resource.archetype.specific.ok_or("app resource must have a specific")?,
                    config:  resource.archetype.config.ok_or("app resource must have a config")?,
                },
                address: resource.address,
                owner: resource.owner.ok_or("app resource must have an owner")?
            };
            Ok(resource)
        }
        else {
            Err("could not get a proper AppKind".into())
        }

    }
}


#[derive(Clone)]
pub struct AppContext
{
    sequence: Option<Arc<IdSeq>>,
    app_tx: mpsc::Sender<AppSliceCommand>,
    comm: StarComm,
    actor_key_seq: Option<ActorKeySeq>,
    resource: AppResource
}

impl AppContext
{
    pub fn new( resource: AppResource, app_tx: mpsc::Sender<AppSliceCommand>, comm: StarComm )->Self
    {
        AppContext{
            actor_key_seq: Option::None,
            app_tx: app_tx,
            sequence: Option::None,
            comm: comm,
            resource: resource
        }
    }

    pub async fn reply( &self, message: &ResourceMessage, payload: Arc<RawPayload> ) {
        let mut reply = message.clone();

        reply.payload = payload;
        reply.to = message.from.reverse();
        reply.from = message.to.reverse();

        // send
    }

    pub async fn forward( &self, message: &ResourceMessage, from: ResourceFrom, to: ResourceTo ) {
        let mut reply = message.clone();
        reply.from = from;
        reply.to = to;
        // send
    }


    pub async fn resource(&mut self)->AppResource {
        self.resource.clone()
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

    pub async fn unique_actor_key_seq(&mut self)->Result<ActorKeySeq,Fail>
    {
        let (request,rx) = Request::new(Empty::new() );
        self.app_tx.send( AppSliceCommand::FetchSequence(request)).await;
        if let seq_id= rx.await??
        {
            let (tx,mut rx) = mpsc::channel(16);
            let actor_key_seq = ActorKeySeq::new(self.resource.key.clone(), seq_id, 0, tx );
            let variant_tx= self.comm.variant_tx.clone();
            tokio::spawn(async move {
                while let Option::Some(actor) = rx.recv().await {
                    let local_location = LocalResourceLocation{
                        resource: ResourceKey::Actor(actor),
                        gathering: Option::None
                    };
                    variant_tx.send( StarVariantCommand::ResourceCommand(ResourceCommand::SignalLocation(local_location))).await;
                }
            } );
            Ok(actor_key_seq)
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

    pub async fn actor_key_seq(&mut self)->Result<ActorKeySeq,Fail> {
        if let Option::None = self.sequence
        {
            self.actor_key_seq= Option::Some(self.unique_actor_key_seq().await?)
        }

        Ok(self.actor_key_seq.as_ref().unwrap().clone())
    }

    pub async fn next_id(&mut self)->Result<Id,Fail>
    {
        Ok(self.seq().await?.next())
    }

    pub async fn next_actor_key(&mut self)->Result<ActorKey,Fail>
    {
        Ok(self.actor_key_seq().await?.next().await)
    }

    pub async fn create_actor_key(&mut self, archetype: ActorArchetype, names: Option<Names>, labels: Option<Labels>) ->Result<ActorKey,Fail>
    {
        let actor_id = self.next_id().await?;

        let actor_key = ActorKey{
            app: self.resource.key.clone(),
            id: actor_id
        };

        let address_part = ResourceAddressPart::Skewer(Skewer::new(ResourceKey::Actor(actor_key.clone()).encode()?.as_str())?);

        let resource = ActorResource{
            key: actor_key.clone(),
            owner: self.resource.owner.clone(),
            archetype: archetype,
            address: ResourceAddress::from_parent(&ResourceType::Actor, &self.resource.address, address_part )?
        };

        let registration = ActorRegistration{
            resource: resource,
            names: names.unwrap_or(vec![]),
            labels: labels.unwrap_or(Labels::new()),
        };

        self.register(registration).await?;

        Ok( actor_key )
    }

    pub async fn register(&mut self, registration: ActorRegistration ) -> Result<(),Fail>
    {
        let registration: ResourceRegistration = registration.into();
        let (request,rx) = Request::new(registration);
        self.comm.variant_tx.send( StarVariantCommand::ResourceCommand(ResourceCommand::Register(request))).await;
        rx.await?;
        Ok(())
    }
}



pub type Raw=Vec<u8>;
