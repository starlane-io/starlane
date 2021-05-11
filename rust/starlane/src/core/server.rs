use tokio::sync::mpsc;

use crate::actor::{ActorKey, Actor, ActorInfo, ActorProfile, NewActor, ActorAssign, ActorKind, ActorKindExt, ActorContext, ActorRef};
use crate::app::{Alert, AppCommandKind, AppKind, AppArchetype, AppMeta, AppCreateResult, AppMessageResult, ActorMessageResult, AppSlice};
use crate::core::{StarCore, StarCoreCommand, StarCoreExt, StarCoreExtKind, AppCommandResult, StarCoreAppMessagePayload};
use crate::error::Error;
use crate::frame::{ActorMessage, AppMessage, Watch, ServerAppPayload, StarMessagePayload, SpaceMessage, SpacePayload };
use crate::star::{ActorCreate, StarSkel, StarCommand, StarKey};
use crate::keys::{AppKey, SubSpaceKey, UserKey};
use crate::message::ProtoMessage;
use std::collections::HashMap;
use crate::label::Labels;
use crate::id::IdSeq;
use std::sync::Arc;
use crate::actor;
use crate::artifact::Artifact;


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
println!("ServerCore received StarCoreAppAssign command");
                                match self.ext.app_ext(&assign.meta.kind)
                                {
                                    Ok(app_ext) => {
                                        let app_slice = AppSlice::new(assign.meta, self.skel.clone(), app_ext );
                                        self.apps.insert( app.clone(), app_slice );
                                    }
                                    Err(error) => {
                                        assign.tx.send(Result::Err(error));
                                    }
                                }
                            }
                            StarCoreAppMessagePayload::Launch(launch) => {

                                unimplemented!()
                                /*
                                let launcher = self.ext.app_launcher(&launch.launch.data.kind);
                                match launcher
                                {
                                    Ok(launcher) => {

                                        unimplemented!()
                                        /*
                                        let result = launcher.launch(&context,app.clone(), launch.create.clone() ).await;

                                        if let Result::Ok(_)=result
                                        {
                                            let app_slice = AppSlice::new();
                                            self.apps.insert( app.clone(), app_slice );
                                        }

                                        launch.tx.send(result);

                                         */
                                    }
                                    Err(error) => {
                                        launch.tx.send(Result::Err(error));
                                    }
                                }
                                 */
                            }

                        }
                    }
println!("StarCore received app message!");
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
    async fn launch(&self, app: &mut AppSlice, archetype: AppArchetype) -> Result<(), AppExtError>;
    async fn actor_create(&self, app: &mut AppSlice, assign: ActorAssign ) -> Result<Box<dyn Actor>,ActorCreateError>;
    async fn app_message( &self, app: &mut AppSlice, message: AppMessage ) ->  Result<(),AppMessageError>;
    async fn actor_message( &self, app: &mut AppSlice, message: ActorMessage ) -> Result<(),ActorMessageResult>;
}

pub enum ActorCreateError
{
    Error(String)
}

pub trait ServerStarCoreExt: StarCoreExt
{
    fn app_ext( &self, kind: &AppKind ) -> Result<Box<dyn AppExt>, AppExtError>;
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
    fn app_ext(&self, kind: &AppKind) -> Result<Box<dyn AppExt>, AppExtError> {
        if *kind == crate::names::TEST_APP_KIND.as_name()
        {
            Ok(Box::new(TestAppCreateExt::new()))
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
    async fn launch(&self, app: &mut AppSlice, archetype: AppArchetype) -> Result<(), AppExtError>
    {
        let actor = app.actor_create(actor::MakeMeAnActor {
            app: app.meta.app.clone(),
            kind: crate::names::TEST_ACTOR_KIND.as_kind(),
            data: Arc::new(vec![]),
            labels: Default::default()
        }).await;

        match actor
        {
            Ok(_) => {
                Ok(())
            }
            Err(err) => {
                Err(AppExtError::Error(err.to_string()))
            }
        }
    }

    async fn actor_create(&self, app: &mut AppSlice, assign: ActorAssign) -> Result<Box<dyn Actor>, ActorCreateError> {
        todo!()
    }

    async fn app_message(&self, app: &mut AppSlice, message: AppMessage) -> Result<(), AppMessageError> {
        todo!()
    }

    async fn actor_message(&self, app: &mut AppSlice, message: ActorMessage) -> Result<(), ActorMessageResult> {
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