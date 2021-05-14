use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{mpsc, Mutex, oneshot};

use crate::actor::{ActorArchetype, ActorAssign, ActorContext, ActorInfo, ActorKey, ActorKind, ActorSpecific, ActorMessage};
use crate::actor;
use crate::app::{ActorMessageResult, Alert, AppArchetype, AppCommandKind, AppContext, AppCreateResult, AppSpecific, AppMessageResult, AppMeta, AppSlice, ConfigSrc, InitData, AppMessage, AppSliceCommand};
use crate::artifact::Artifact;
use crate::core::{AppCommandResult, AppLaunchError, StarCore, StarCoreAppMessagePayload, StarCoreCommand, StarCoreExt, StarCoreExtKind};
use crate::error::Error;
use crate::frame::{ServerAppPayload, SpaceMessage, SpacePayload, StarMessagePayload, Watch};
use crate::frame::ServerPayload::AppLaunch;
use crate::id::{Id, IdSeq};
use crate::keys::{AppKey, SubSpaceKey, UserKey, ResourceKey};
use crate::resource::{Labels, ResourceRegistration};
use crate::message::{ProtoMessage, Fail};
use crate::star::{ActorCreate, StarCommand, StarKey, StarSkel, Request, LocalResourceLocation};
use tokio::sync::oneshot::error::RecvError;

pub struct ServerStarCore
{
    pub skel: StarSkel,
    pub apps: HashMap<AppKey,mpsc::Sender<AppSliceCommand>>,
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
                                match self.ext.app_ext(&assign.payload.specific)
                                {
                                    Ok(app_ext) => {
                                        let app_slice = AppSlice::new(assign.payload, self.skel.comm(), app_ext ).await;
                                        self.apps.insert( app.clone(), app_slice );
                                        assign.tx.send(Result::Ok(()));
                                    }
                                    Err(error) => {
                                        assign.tx.send(Result::Err(error));
                                    }
                                }
                            }
                            StarCoreAppMessagePayload::Launch(launch) => {
                                if let Option::Some(app) = self.apps.get_mut(&launch.payload.key )
                                {
                                    let (request,rx) = Request::new(launch.payload.archetype);
                                    app.send( AppSliceCommand::Launch(request)).await;

                                }
                                else
                                {
                                    launch.tx.send( Result::Err(Fail::ResourceNotFound(ResourceKey::App(app))));
                                }
                            }

                        }
                    }
                }
                StarCoreCommand::HasResource(request) => {
                    let resource = request.payload.clone();
                    match &request.payload
                    {
                        ResourceKey::Actor(actor) => {
                            if let Option::Some(app) = self.apps.get_mut(&actor.app) {
                               let (new_request,mut rx) = Request::new(resource.clone() );
                               app.send(AppSliceCommand::HasActor(new_request)).await;
                                tokio::spawn(async move{
                                    match rx.await
                                    {
                                        Ok(result) => {
                                            request.tx.send(result);
                                        }
                                        Err(_) => {
                                            request.tx.send(Err(Fail::Unexpected));
                                        }
                                    }
                                });
                            } else {
                                request.tx.send( Err(Fail::ResourceNotFound(resource)));
                            }
                        }
                        _ => {
                            request.tx.send( Err(Fail::ResourceNotFound(resource)));
                        }
                    }
                }
            }
        }
    }
}


#[async_trait]
pub trait AppExt : Sync+Send
{
    fn set_context( &mut self, context: AppContext );
    async fn launch(&mut self, archetype: AppArchetype) -> Result<(), Fail>;
    async fn app_message( &mut self, message: AppMessage ) ->  Result<(),Fail>;
    async fn actor_message( &mut self, message: ActorMessage ) -> Result<(),Fail>;
}

pub trait ServerStarCoreExt: StarCoreExt
{
    fn app_ext(&self, kind: &AppSpecific) -> Result<Box<dyn AppExt>, Fail>;
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
    fn app_ext(&self, spec: &AppSpecific) -> Result<Box<dyn AppExt>, Fail> {
        if *spec == crate::names::TEST_APP_SPEC.as_name()
        {
            Ok(Box::new(TestAppCreateExt::new()))
        }
        else {
            Err(Fail::DoNotKnowSpecific(spec.clone()))
        }
    }
}


pub enum AppExtFactoryError
{
    DoNotServerAppKind(AppSpecific)
}

pub struct TestAppCreateExt
{
    context: Option<AppContext>
}

impl TestAppCreateExt
{
    pub fn new()->Self
    {
        TestAppCreateExt {
            context: Option::None
        }
    }
}

impl TestAppCreateExt
{
    pub fn context(&self) -> Result<AppContext,Error>
    {
        self.context.clone().ok_or("AppSlice: context not set".into() )
    }
}

#[async_trait]
impl AppExt for TestAppCreateExt
{
    fn set_context(&mut self, context: AppContext) {
        self.context = Option::Some(context);
    }


    async fn launch(&mut self, archetype: AppArchetype) -> Result<(), Fail>
    {
        let meta = self.context()?.meta().await;
        let mut archetype = ActorArchetype::new( ActorKind::Single, crate::names::TEST_ACTOR_SPEC.clone(), meta.owner );
        archetype.name = Option::Some("main".to_string());

        let actor = self.context()?.create_actor_key().await?;

        Ok(())
    }

    async fn app_message(&mut self, message: AppMessage) -> Result<(), Fail> {
        todo!()
    }

    async fn actor_message(&mut self, message: ActorMessage) -> Result<(), Fail> {
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

