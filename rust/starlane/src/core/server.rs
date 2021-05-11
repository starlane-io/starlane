use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{mpsc, Mutex, oneshot};

use crate::actor::{Actor, ActorArchetype, ActorAssign, ActorContext, ActorInfo, ActorKey, ActorKind, ActorKindExt, ActorProfile, ActorRef, NewActor};
use crate::actor;
use crate::app::{ActorMessageResult, Alert, AppArchetype, AppCommandKind, AppContext, AppCreateResult, AppKind, AppMessageResult, AppMeta, AppSlice, AppSliceInner, ConfigSrc, InitData};
use crate::artifact::Artifact;
use crate::core::{AppCommandResult, AppLaunchError, StarCore, StarCoreAppMessagePayload, StarCoreCommand, StarCoreExt, StarCoreExtKind};
use crate::error::Error;
use crate::frame::{ActorMessage, AppMessage, ServerAppPayload, SpaceMessage, SpacePayload, StarMessagePayload, Watch};
use crate::frame::ServerPayload::AppLaunch;
use crate::id::{Id, IdSeq};
use crate::keys::{AppKey, SubSpaceKey, UserKey};
use crate::label::Labels;
use crate::message::ProtoMessage;
use crate::star::{ActorCreate, StarCommand, StarKey, StarSkel};

pub struct ServerStarCore
{
    pub skel: StarSkel,
    pub apps: HashMap<AppKey,AppSlice>,
    pub ext: Box<dyn ServerStarCoreExt>,
    pub supervisor: Option<StarKey>,
    pub core_rx: mpsc::Receiver<StarCoreCommand>
}

impl ServerStarCore
{
    pub fn new(skel: StarSkel, ext: Box<dyn ServerStarCoreExt>, core_rx: mpsc::Receiver<StarCoreCommand>)->Self
    {
        ServerStarCore
        {
            skel: skel,
            apps: HashMap::new(),
            ext: ext,
            supervisor: Option::None,
            core_rx: core_rx
        }
    }
}

impl ServerStarCore
{
    pub async fn alert( &mut self, alert: Alert )
    {
        // not sure what to do with alerts yet
    }

    pub async fn add( &mut self, key: ActorKey, actor: Box<dyn Actor>, labels: Option<Labels>)
    {
println!("ADD.... ACTOR....");
        /*
        if let Option::Some(app) = self.apps.get_mut(&key.app )
        {
            //...
            app.actors.insert( key.clone(), actor );
        }
        else {
//            return Err(format!("App {} is not being hosted", &key.app).into() );
        }


         */
 //       Ok(())
    }
}
#[async_trait]
impl StarCore for ServerStarCore
{
    async fn run(&mut self)
    {
        while let Option::Some(command) = self.core_rx.recv().await
        {
            match command
            {
                StarCoreCommand::SetSupervisor(supervisor) => {
                    self.supervisor = Option::Some(supervisor);
                }
                StarCoreCommand::Watch(_) => {}

                StarCoreCommand::AppMessage(message) => {
                    if let Option::Some(supervisor) = &self.supervisor
                    {
                        let app= message.app.clone();
                        match message.payload
                        {
                            StarCoreAppMessagePayload::None => {}
                            StarCoreAppMessagePayload::Assign(assign ) => {
                                match self.ext.app_ext(&assign.meta.kind)
                                {
                                    Ok(app_ext) => {
                                        let app_slice = AppSlice::new(assign.meta, self.skel.clone(), app_ext );
                                        self.apps.insert( app.clone(), app_slice );
                                        assign.tx.send(Result::Ok(()));
                                    }
                                    Err(error) => {
                                        assign.tx.send(Result::Err(error));
                                    }
                                }
                            }
                            StarCoreAppMessagePayload::Launch(launch) => {
                                if let Option::Some(app) = self.apps.get_mut(&launch.app.key )
                                {
                                    let mut context = app.context();
                                    let ext = app.ext().await;
                                    let result = ext.launch(&mut context,launch.app.archetype).await;
                                    match result
                                    {
                                        Ok(_) => {
                                            launch.tx.send(Result::Ok(()));
                                        }
                                        Err(error) => {

                                            launch.tx.send(Result::Err(error));
                                        }
                                    }
                                }
                                else
                                {
                                    launch.tx.send( Result::Err(AppLaunchError::Error("Cannot findn app".to_string())));
                                }
                            }

                        }
                    }
                }
                _ => {
                eprintln!("unexpected star command");
            }
            }
        }
    }
}


#[async_trait]
pub trait AppExt : Sync+Send
{
    async fn launch(&self, context: &mut AppContext, archetype: AppArchetype) -> Result<(), AppLaunchError>;
    async fn app_message( &self, context: &mut AppContext, message: AppMessage ) ->  Result<(),AppMessageError>;

    async fn actor_create( &self, context: &mut ActorContext, archetype: ActorArchetype ) ->  Result<Arc<dyn Actor>,ActorCreateError>;
    async fn actor_message( &self, context: &mut ActorContext , message: ActorMessage ) -> Result<(),ActorMessageResult>;
}

pub enum ActorCreateError
{
    Error(String)
}

impl ActorCreateError
{
    pub fn to_string(&self)->String
    {
        match self
        {
            ActorCreateError::Error(error) => {error.to_string()}
        }
    }
}

pub trait ServerStarCoreExt: StarCoreExt
{
    fn app_ext( &self, kind: &AppKind ) -> Result<Arc<dyn AppExt>, AppExtError>;
}

pub enum AppExtError
{
    DoNotKnowAppKind(AppKind),
    Error(String)
}

pub enum AppMessageError
{
    Error(String)
}

pub enum ActorMessageError
{
    Error(String)
}



pub struct ExampleServerStarCoreExt
{
    pub skel: StarSkel,
}

impl ExampleServerStarCoreExt
{
    pub fn new( skel: StarSkel )->Self
    {
       ExampleServerStarCoreExt{
           skel: skel
       }
    }
}

#[async_trait]
impl StarCoreExt for ExampleServerStarCoreExt
{
}

#[async_trait]
impl ServerStarCoreExt for ExampleServerStarCoreExt
{
    fn app_ext(&self, kind: &AppKind) -> Result<Arc<dyn AppExt>, AppExtError> {
        if *kind == crate::names::TEST_APP_KIND.as_name()
        {
            Ok(Arc::new(TestAppCreateExt::new()))
        }
        else {
            Err(AppExtError::DoNotKnowAppKind(kind.clone()))
        }
    }
}


pub enum AppExtFactoryError
{
    DoNotServerAppKind(AppKind)
}

pub struct TestAppCreateExt
{
}

impl TestAppCreateExt
{
    pub fn new()->Self
    {
        TestAppCreateExt {}
    }
}

#[async_trait]
impl AppExt for TestAppCreateExt
{
    async fn launch(&self, context: &mut AppContext, archetype: AppArchetype) -> Result<(), AppLaunchError>
    {
        let meta = context.meta().await;
        let actor = context.actor_create(ActorArchetype {
            owner: meta.owner,
            kind: crate::names::TEST_ACTOR_KIND.as_kind(),
            config: ConfigSrc::None,
            init: InitData::None,
            labels: Labels::new()
        }).await;

        //kind: crate::names::TEST_ACTOR_KIND.as_kind(),
        match actor
        {
            Ok(_) => {
                Ok(())
            }
            Err(err) => {
                Err(AppLaunchError::Error(err.to_string()))
            }
        }
    }

    async fn app_message(&self, app: &mut AppContext, message: AppMessage) -> Result<(), AppMessageError> {
        todo!()
    }

    async fn actor_create(&self, context: &mut ActorContext, archetype: ActorArchetype) -> Result<Arc<dyn Actor>, ActorCreateError> {
        Ok(Arc::new(TestActor::new()))
    }

    async fn actor_message(&self, app: &mut ActorContext, message: ActorMessage) -> Result<(), ActorMessageResult> {
        todo!()
    }
}


pub struct TestActor
{
}

impl TestActor
{
    pub fn new()->Self
    {
        TestActor{}
    }
}

#[async_trait]
impl Actor for TestActor
{
    async fn handle_message(&mut self, context: &ActorContext, message: ActorMessage) {
        todo!()
    }
}
