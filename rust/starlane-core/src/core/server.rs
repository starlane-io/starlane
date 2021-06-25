use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{mpsc, oneshot, Mutex};

use crate::actor;
use crate::error::Error;
use crate::id::{Id, IdSeq};
use crate::keys::{AppKey, ResourceKey, SubSpaceKey, UserKey};
use crate::message::resource::Message;
use crate::message::{Fail, ProtoStarMessage};
use crate::star::{ActorCreate, LocalResourceLocation, Request, StarCommand, StarKey, StarSkel};
use tokio::sync::oneshot::error::RecvError;

/*
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
            unimplemented!()
        }
    }
}


#[async_trait]
pub trait AppExt : Sync+Send
{
    fn set_context( &mut self, context: AppContext );
    async fn launch(&mut self, archetype: AppArchetype) -> Result<(), Fail>;
   // async fn app_message(&mut self, message: Message) ->  Result<(),Fail>;
   // async fn actor_message(&mut self, message: Message) -> Result<(),Fail>;
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
        let mut archetype = ActorArchetype::new( ActorKind::Single, crate::names::TEST_ACTOR_SPEC.clone(), ConfigSrc::Artifact(crate::names::TEST_ACTOR_CONFIG_ARTIFACT.clone()) );
        let actor = self.context()?.create_actor_key(archetype, Option::Some(vec!["main".to_string()]), Option::None).await?;

        Ok(())
    }

    /*
    async fn app_message(&mut self, message: Message) -> Result<(), Fail> {
        todo!()
    }

    async fn actor_message(&mut self, message: Message) -> Result<(), Fail> {
        todo!()
    }

     */
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



 */
