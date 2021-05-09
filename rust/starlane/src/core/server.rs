use tokio::sync::mpsc;

use crate::actor::{ActorKey, Actor, ActorInfo, ActorProfile};
use crate::app::{Alert, AppCommandKind, AppKind, AppCreateData, AppInfo, AppCreateResult, AppMessageResult, ActorMessageResult, AppSlice};
use crate::core::{StarCore, StarCoreCommand, StarCoreExt, StarCoreExtKind, AppCommandResult};
use crate::error::Error;
use crate::frame::{ActorMessage, AppCreate, AppMessage, Watch, AppMessagePayload, StarMessagePayload, SpaceMessage, SpacePayload, RequestMessage};
use crate::star::{ActorCreate, StarSkel, StarCommand, StarKey};
use crate::keys::{AppKey, SubSpaceKey, UserKey};
use crate::message::ProtoMessage;
use std::collections::HashMap;
use crate::label::Labels;



pub struct ServerStarCore
{
    pub skel: StarSkel,
    pub apps: HashMap<AppKey,AppSlice>,
    pub ext: Box<dyn ServerStarCoreExt>,
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
                StarCoreCommand::Watch(_) => {}

                StarCoreCommand::AppMessage(message) => {
                    match message.message.payload
                    {
                        AppMessagePayload::None => {}
                        AppMessagePayload::Launch(create) => {
//                                let result = self.ext.app_create_factory_ext( &self.skel.info.kind );
                            unimplemented!();

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

struct Registration
{
    pub actor: Box<dyn Actor>,
    pub labels: Option<Labels>
}

#[derive(Clone)]
pub struct AppContext
{
    pub info: AppInfo,
    supervisor: StarKey,
    sub_space: SubSpaceKey,
    owner: UserKey,
    star_tx: mpsc::Sender<StarCommand>
}

impl AppContext
{
    pub async fn send_actor_message(&self, message: ActorMessage )
    {
        self.star_tx.send( StarCommand::ActorMessage( message )).await;
    }

    pub async fn register_actor(&self, profile: ActorProfile )
    {
        let mut proto = ProtoMessage::new();
        proto.to = Option::Some(self.supervisor.clone());
        proto.payload = StarMessagePayload::Space(SpaceMessage{
            sub_space: self.sub_space.clone(),
            user: self.owner.clone(),
            payload: SpacePayload::Request(RequestMessage::ActorRegister(profile))
        });
        self.star_tx.send( StarCommand::SendProtoMessage(proto) ).await;
    }

    pub async fn unregister_actor(&self, actor: ActorKey )
    {
        let mut proto = ProtoMessage::new();
        proto.to = Option::Some(self.supervisor.clone());
        proto.payload = StarMessagePayload::Space(SpaceMessage{
            sub_space: self.sub_space.clone(),
            user: self.owner.clone(),
            payload: SpacePayload::Request(RequestMessage::ActorUnRegister(actor))
        });
        self.star_tx.send( StarCommand::SendProtoMessage(proto) ).await;
    }

}

pub trait AppCreateExt
{
    fn create( &self, context: &AppContext, key: AppKey, data: AppCreateData ) -> AppCreateResult;
}

pub trait AppExt
{
    fn app_message( &self, context: &AppContext, message: AppMessage ) -> AppMessageResult;
    fn actor_message( &self, context: &AppContext, message: ActorMessage ) -> ActorMessageResult;
}

pub trait ServerStarCoreExt: StarCoreExt
{
    fn app_create_factory_ext(&self, kind: &AppKind) -> Result<Box<dyn AppCreateExt>,AppCreateExtFactoryError>;
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
    fn app_create_factory_ext(&self, kind: &AppKind) -> Result<Box<dyn AppCreateExt>,AppCreateExtFactoryError> {
        match kind.as_str()
        {
            "test"=>Ok(Box::new(TestAppCreateExt::new())),
            _ => {
                Err(AppCreateExtFactoryError::DoNotServerAppKind(kind.clone()))
            }
        }
    }
}

pub enum AppCreateExtFactoryError
{
    DoNotServerAppKind(AppKind)
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

impl AppCreateExt for TestAppCreateExt
{
    fn create(&self, context: &AppContext, key: AppKey, data: AppCreateData) -> AppCreateResult {
        let actor = TestActor::new();
        AppCreateResult::Ok
    }
}

#[derive(Clone)]
pub struct ActorContext
{
    pub info: ActorInfo,
    pub app_context: AppContext
}

pub struct TestActor
{
}

impl TestActor
{
    pub fn new()->Self
    {
        unimplemented!()
    }
}

#[async_trait]
impl Actor for TestActor
{
    async fn handle_message(&mut self, context: &ActorContext, message: ActorMessage) {
        todo!()
    }
}