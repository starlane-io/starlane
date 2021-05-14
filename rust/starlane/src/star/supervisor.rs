use core::option::Option;
use core::option::Option::{None, Some};
use core::result::Result;
use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use tokio::sync::mpsc::error::SendError;

use crate::actor::{ActorKey, ActorLocation};
use crate::app::{AppMeta, AppLocation, AppStatus, AppReadyStatus, AppPanicReason, AppArchetype, InitData, AppCreateResult, App};
use crate::error::Error;
use crate::frame::{ActorLookup, AppNotifyCreated, AssignMessage, Frame, Reply, SpaceMessage, SpacePayload, StarMessage, StarMessagePayload, ServerAppPayload, SpaceReply, StarMessageCentral, SimpleReply, SupervisorPayload, StarMessageSupervisor, ServerPayload, StarPattern, FromReply};
use crate::keys::{AppKey, UserKey};
use crate::logger::{Flag, Log, StarFlag, StarLog, StarLogPayload};
use crate::message::{MessageExpect, ProtoMessage, MessageExpectWait, Fail};
use crate::star::{StarCommand, StarSkel, StarInfo, StarKey, StarVariant, StarVariantCommand, StarKind, RegistryBacking, RegistryBackingSqlLite};
use tokio::sync::oneshot::Receiver;
use tokio::sync::oneshot::error::RecvError;
use crate::star::supervisor::SupervisorCommand::AppAssign;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::{oneshot, mpsc};
use rusqlite::{Connection,params};
use std::str::FromStr;
use serde::{Deserialize, Serialize};
use crate::resource::{RegistryAction, Registry, FieldSelection};
use std::future::Future;

pub enum SupervisorCommand
{
    Pledge,
    SetAppStatus(SetAppStatus),
    AppAssign(App),
    AppLaunch(AppKey),
    SetAppServerStatus(SetAppServerStatus)
}

pub struct SetAppServerStatus
{
    pub app: AppKey,
    pub server: StarKey,
    pub status: AppServerStatus
}



pub struct SetAppStatus
{
    pub app: AppKey,
    pub status: AppStatus
}

pub struct SupervisorVariant
{
    skel: StarSkel,
    backing: Box<dyn SupervisorManagerBacking>,
    registry: Box<dyn RegistryBacking>
}

impl SupervisorVariant
{
    pub async fn new(data: StarSkel) ->Self
    {
        SupervisorVariant {
            skel: data.clone(),
            backing: Box::new(SupervisorStarVariantBackingSqLite::new().await ),
            registry: Box::new(RegistryBackingSqlLite::new().await ),
        }
    }
}

impl SupervisorVariant
{
    async fn pledge( &mut self )
    {
        let mut proto = ProtoMessage::new();
        proto.to = Option::Some(StarKey::central());
        proto.payload = StarMessagePayload::Central(StarMessageCentral::Pledge(self.skel.info.kind.clone()));
        proto.expect = MessageExpect::RetryUntilOk;
        let rx = proto.get_ok_result().await;
        self.skel.star_tx.send(StarCommand::SendProtoMessage(proto)).await;

        if self.skel.flags.check(Flag::Star(StarFlag::DiagnosePledge))
        {
            self.skel.logger.log( Log::Star( StarLog::new(&self.skel.info, StarLogPayload::PledgeSent )));
            let mut data = self.skel.clone();
            tokio::spawn(async move {
                let payload = rx.await;
                if let Ok(StarMessagePayload::Reply(SimpleReply::Ok(_))) = payload
                {
                    data.logger.log( Log::Star( StarLog::new( &data.info, StarLogPayload::PledgeOkRecv )))
                }
            });
        }
    }

    pub fn unwrap(&self, result: Result<(), SendError<StarCommand>>)
    {
        match result
        {
            Ok(_) => {}
            Err(error) => {
                eprintln!("could not send starcommand from manager to star: {}", error);
            }
        }
    }

    pub async fn reply_ok(&self, message: StarMessage)
    {
        let mut proto = message.reply(StarMessagePayload::Reply(SimpleReply::Ok(Reply::Empty)));
        let result = self.skel.star_tx.send(StarCommand::SendProtoMessage(proto)).await;
        self.unwrap(result);
    }

    pub async fn reply_error(&self, mut message: StarMessage, error_message: String )
    {
        message.reply(StarMessagePayload::Reply(SimpleReply::Fail(Fail::Error(error_message.to_string()))));
        let result = self.skel.star_tx.send(StarCommand::Frame(Frame::StarMessage(message))).await;
        self.unwrap(result);
    }

}

#[async_trait]
impl StarVariant for SupervisorVariant
{
    async fn handle(&mut self, command: StarVariantCommand) {
        match command
        {
            StarVariantCommand::Init => {
                self.pledge().await;
            }
            StarVariantCommand::SupervisorCommand(supervisor_command) => {
                match supervisor_command
                {
                    SupervisorCommand::Pledge => {
                        self.pledge().await;
                    }
                    SupervisorCommand::SetAppStatus(set_app_status) => {
                        self.backing.set_app_status(set_app_status.app.clone(), set_app_status.status.clone());
                    }
                    SupervisorCommand::AppAssign(app) => {
                        let servers = self.backing.select_servers(StarPattern::Any).await;
                        if !servers.is_empty()
                        {
                            for server in servers
                            {
                                self.backing.set_app_server_status(app.key.clone(), server.clone(), AppServerStatus::Assigning );
                                let mut proto = ProtoMessage::new();
                                proto.to(server.clone());
                                proto.payload = StarMessagePayload::Space(SpaceMessage{
                                    sub_space: app.key.sub_space.clone(),
                                    user: UserKey::hyper_user(),
                                    payload: SpacePayload::Server(ServerPayload::AppAssign(app.meta()))
                                });
                                let result = proto.get_ok_result().await;
                                self.skel.star_tx.send( StarCommand::SendProtoMessage(proto)).await;
                                let manager_tx = self.skel.variant_tx.clone();
                                let app = app.clone();
                                let server= server.clone();
                                tokio::spawn( async move {
                                    match result.await
                                    {
                                        Ok(_) => {
                                            manager_tx.send(StarVariantCommand::SupervisorCommand(SupervisorCommand::SetAppServerStatus(SetAppServerStatus{
                                                app: app.key.clone(),
                                                server: server.clone(),
                                                status: AppServerStatus::Ready
                                            }))).await;
                                        }
                                        Err(error) => {
                                            eprintln!("{}",error);

                                        }
                                    }
                                } );

                            }
                        }
                        else {
                            // leave in Waiting state
                        }
                    }
                    SupervisorCommand::SetAppServerStatus(set_status) => {
                        self.backing.set_app_server_status(set_status.app.clone(),set_status.server.clone(), set_status.status.clone() ).await;
                        if self.backing.get_app_status(&set_status.app).await == AppStatus::Pending
                        {
                            self.backing.set_app_status(set_status.app.clone(),  AppStatus::Launching ).await;
                            self.skel.variant_tx.send(StarVariantCommand::SupervisorCommand(SupervisorCommand::AppLaunch(set_status.app.clone()))).await;
                        }
                    }
                    SupervisorCommand::AppLaunch(app_key) => {
                        let archetype = self.backing.get_application(&app_key).await;
                        let servers = self.backing.get_servers_for_app(&app_key).await;

                        if archetype.is_none()
                        {
                            eprintln!("cannot find archetype for app: {}",app_key);
                        }

                        if servers.is_empty()
                        {
                            eprintln!("cannot select a server for app: {}",app_key);
                            return;
                        }

                        let server = servers.get(0).cloned().unwrap();

                        if let Option::Some(archetype) = archetype
                        {
                                let app = App {
                                    key: app_key.clone(),
                                    archetype: archetype.clone()
                                };
                                let mut proto = ProtoMessage::new();
                                proto.to(server.clone());
                                proto.payload = StarMessagePayload::Space(SpaceMessage {
                                    sub_space: app.key.sub_space.clone(),
                                    user: UserKey::hyper_user(),
                                    payload: SpacePayload::Server(ServerPayload::AppLaunch(app))
                                });
                                let result = proto.get_ok_result().await;
                                self.skel.star_tx.send(StarCommand::SendProtoMessage(proto)).await;

                                let manager_tx = self.skel.variant_tx.clone();
                                tokio::spawn(async move {
                                    match result.await
                                    {
                                        Ok(_) => {
                                            manager_tx.send(StarVariantCommand::SupervisorCommand(SupervisorCommand::SetAppStatus(SetAppStatus {
                                                app: app_key,
                                                status: AppStatus::Ready
                                            }))).await;
                                        }
                                        Err(error) => {
                                            eprintln!("{}", error);
                                        }
                                    }
                                });
                        }
                    }
                }
            }
            StarVariantCommand::StarMessage(star_message) =>
            {
                match &star_message.payload
                {
                    StarMessagePayload::Space(space_message) => {
                        match &space_message.payload
                        {
                            SpacePayload::Supervisor(supervisor_payload) => {
                                match supervisor_payload
                                {
                                    SupervisorPayload::AppCreate(archetype) => {
                                        let app_key = AppKey::new(space_message.sub_space.clone());
                                        self.backing.set_application(app_key.clone(), archetype.clone() );
                                        self.backing.set_app_status(app_key.clone(), AppStatus::Pending);
                                        let proto = star_message.reply( StarMessagePayload::Reply(SimpleReply::Ok(Reply::Empty)));
                                        self.skel.star_tx.send(StarCommand::SendProtoMessage(proto)).await;
                                        let app = App{
                                            key: app_key,
                                            archetype: archetype.clone()
                                        };
                                        self.skel.variant_tx.send( StarVariantCommand::SupervisorCommand(SupervisorCommand::AppAssign(app))).await;
                                    }
                                    SupervisorPayload::AppSequenceRequest(app_key) => {
println!("AppSEquenceRequest!");
                                        let index = self.backing.app_sequence_next(app_key).await;
                                        match index
                                        {
                                            Ok(index) => {
                                                let reply = star_message.reply(StarMessagePayload::Reply(SimpleReply::Ok(Reply::Seq(index))));
                                                self.skel.star_tx.send(StarCommand::SendProtoMessage(reply)).await;
                                            }
                                            Err(error) => {
                                                let reply = star_message.reply(StarMessagePayload::Reply(SimpleReply::Fail(Fail::Error("could not generate sequence".to_string()))));
                                                self.skel.star_tx.send(StarCommand::SendProtoMessage(reply)).await;
                                            }
                                        }
                                    }
                                    SupervisorPayload::ActorUnRegister(_) => {}
                                    SupervisorPayload::ActorStatus(_) => {}
                                    SupervisorPayload::Register(register) => {
                                        let result = self.registry.register(register.clone() ).await;
                                        self.skel.comm().reply_result(star_message, Reply::from_result(result) );
                                    }
                                    SupervisorPayload::Select(selector) => {
                                        let mut selector = selector.clone();
                                        selector.add(FieldSelection::Space(space_message.sub_space.space.clone()));
                                        selector.add(FieldSelection::SubSpace(space_message.sub_space.clone()));
                                        let result = self.registry.select(selector).await;
                                        self.skel.comm().reply_result(star_message,Reply::from_result(result)).await;
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    StarMessagePayload::Supervisor(star_message_supervisor)=> {
                            match star_message_supervisor
                            {
                                StarMessageSupervisor::Pledge(kind) => {
                                    let info = StarInfo::new(star_message.from.clone(),kind.clone() );
                                    self.backing.add_server(info).await;
                                    self.reply_ok(star_message.clone()).await;
                                    if self.skel.flags.check( Flag::Star(StarFlag::DiagnosePledge )) {

                                        self.skel.logger.log( Log::Star(StarLog::new(&self.skel.info, StarLogPayload::PledgeRecv )));
                                    }
                                }
                                StarMessageSupervisor::Register(registration) => {
                                    let result = self.registry.register(registration.clone()).await ;
                                    self.skel.comm().reply(star_message, result).await;
                                }
                            }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    /*
    async fn handle(&mut self, command: StarManagerCommand)  {

        match &command
        {

           StarManagerCommand::Init => {
               self.pledge().await;
           }
           StarManagerCommand::StarMessage(message)=>{
              match &message.payload
              {
                  StarMessagePayload::Central(central) => {

                      match central
                      {
                          StarMessageCentral::Pledge(_) => {
                              self.backing.add_server(message.from.clone());
                              self.reply_ok(message.clone()).await;
                              if self.skel.flags.check( Flag::Star(StarFlag::DiagnosePledge )) {

                                  self.skel.logger.log( Log::Star(StarLog::new(&self.skel.info, StarLogPayload::PledgeRecv )));
                              }
                          }
                      }
                  }
                  StarMessagePayload::Space(space_message) =>
                  {
                      match &space_message.payload
                      {
                      /*    SpacePayload::Assign(assign) => {
                              match assign
                              {
                                  AssignMessage::App(launch) => {

                                  }
                              }

                       */
                          SpacePayload::Supervisor(payload) => {
                              match payload
                              {
                                  SupervisorPayload::AppSequenceRequest(_) => {
                                      unimplemented!()
                                  }
                                  SupervisorPayload::ActorRegister(_) => {
                                      unimplemented!();
                                  }
                                  SupervisorPayload::ActorUnRegister(_) => {
                                      unimplemented!();
                                  }
                                  SupervisorPayload::ActorStatus(_) => {
                                      unimplemented!();
                                  }
                              }

                          }
                          _ => {}
                      }

                      }

                  StarMessagePayload::None => {}
                  StarMessagePayload::Reply(_) => {}
              }

              }
            StarManagerCommand::StarSkel(_) => {}
            StarManagerCommand::CoreRequest(_) => {}
            StarManagerCommand::CentralCommand(_) => {}
            StarManagerCommand::SupervisorCommand(_) => {}
            StarManagerCommand::ServerCommand(_) => {}
        }



    }}
     */
}



impl SupervisorVariant
{
    async fn handle_message(&mut self, message: StarMessage) {

    }
}

pub struct SupervisorManagerBackingDefault
{
    data: StarSkel,
    servers: Vec<StarKey>,
    server_select_index: usize,
    applications: HashMap<AppKey, AppArchetype>,
    actor_location: HashMap<ActorKey, ActorLocation>,
    app_status: HashMap<AppKey,AppStatus>,
    app_server_status: HashMap<(AppKey,StarKey),AppServerStatus>,
    app_to_servers: HashMap<AppKey,HashSet<StarKey>>,
    app_sequence: HashMap<AppKey,AtomicU64>
}

impl SupervisorManagerBackingDefault
{
    pub fn new(data: StarSkel) ->Self
    {
        SupervisorManagerBackingDefault {
            data: data,
            servers: vec![],
            server_select_index: 0,
            applications: HashMap::new(),
            actor_location: HashMap::new(),
            app_status: HashMap::new(),
            app_server_status: HashMap::new(),
            app_to_servers: HashMap::new(),
            app_sequence: HashMap::new()
        }
    }
}




#[async_trait]
pub trait SupervisorManagerBacking: Send+Sync
{
    async fn add_server( &mut self, info: StarInfo ) -> Result<(),Error>;
    async fn select_servers(&mut self, pattern: StarPattern ) -> Vec<StarKey>;

    async fn set_application(&mut self, app: AppKey, archetype: AppArchetype) -> Result<(),Error>;
    async fn get_application(&mut self, app: &AppKey ) -> Option<AppArchetype>;
    async fn set_app_server_status(&mut self, app: AppKey, server: StarKey, status: AppServerStatus) -> Result<(),Error>;
    async fn get_app_server_status(&mut self, app: &AppKey, server: &StarKey) ->AppServerStatus;
    async fn set_app_status(&mut self, app: AppKey, status: AppStatus )-> Result<(),Error>;
    async fn get_app_status(&mut self, app: &AppKey) -> AppStatus;
    async fn add_app_to_server(&mut self, app: AppKey, server: StarKey)-> Result<(),Error>;
    async fn get_servers_for_app(&mut self, app: &AppKey ) -> Vec<StarKey>;

    async fn app_sequence_next(&mut self, app: &AppKey ) -> Result<u64,Error>;

    async fn set_actor_location(&mut self, location: ActorLocation)-> Result<(),Error>;
    async fn get_actor_location(&self, actor: &ActorKey) -> Option<ActorLocation>;
}


#[derive(Clone,Serialize,Deserialize)]
pub enum AppServerStatus
{
    Unknown,
    Assigning,
    Ready
}


impl fmt::Display for AppServerStatus{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = match self {
            AppServerStatus::Unknown => "Unknown".to_string(),
            AppServerStatus::Assigning => "Assigning".to_string(),
            AppServerStatus::Ready =>  "Ready".to_string()
        };
        write!(f, "{}",r)
    }
}

pub struct SupervisorStarVariantBackingSqLite
{
    pub supervisor_db: mpsc::Sender<SupervisorDbRequest>,
    pub registry: mpsc::Sender<RegistryAction>
}

impl SupervisorStarVariantBackingSqLite
{
    pub async fn new()->Self
    {
        SupervisorStarVariantBackingSqLite {
            supervisor_db: SupervisorDb::new().await,
            registry: Registry::new().await

        }
    }

    pub fn handle( &self, result: Result<Result<SupervisorDbResult,Error>,RecvError>)->Result<(),Error>
    {
        match result
        {
            Ok(ok) => {
                match ok{
                    Ok(_) => {
                        Ok(())
                    }
                    Err(error) => {
                        Err(error)
                    }
                }
            }
            Err(error) => {
                Err(error.into())
            }
        }
    }
}

#[async_trait]
impl SupervisorManagerBacking for SupervisorStarVariantBackingSqLite
{
    async fn add_server(&mut self, info: StarInfo ) -> Result<(), Error> {
        let (request,rx) = SupervisorDbRequest::new( SupervisorDbCommand::AddServer(info));
        self.supervisor_db.send( request ).await;
        self.handle(rx.await)
    }

    async fn select_servers(&mut self, pattern: StarPattern) -> Vec<StarKey> {
println!("SELECT SERVERS!");
        let (request,rx) = SupervisorDbRequest::new( SupervisorDbCommand::StarSelect(pattern));
        self.supervisor_db.send( request ).await;
        match rx.await
        {
            Ok(result) => {
                match result
                {
                    Ok(ok) => {
                        match ok
                        {
                            SupervisorDbResult::Servers(servers) => {

                                println!("rtn servers: {}", servers.len() );
                                servers
                            }
                            _ => vec![]
                        }
                    }
                    Err(err) => {
                        eprintln!("{}",err);
                        vec![]
                    }
                }
            }
            Err(err) => {
                eprintln!("{}",err);
                vec![]
            }
        }
    }

    async fn set_application(&mut self, app: AppKey, archetype: AppArchetype) -> Result<(), Error> {
        let (request,rx) = SupervisorDbRequest::new( SupervisorDbCommand::SetAppArchetype(app, archetype));
        self.supervisor_db.send( request ).await;
        self.handle(rx.await)
    }

    async fn get_application(&mut self, app: &AppKey) -> Option<AppArchetype> {
        let (request,rx) = SupervisorDbRequest::new( SupervisorDbCommand::GetAppArchetype(app.clone()));
        self.supervisor_db.send( request ).await;
        match rx.await
        {
            Ok(result) => {
                match result
                {
                    Ok(ok) => {
                        match ok
                        {
                            SupervisorDbResult::AppArchetype(archetype) => {
                                Option::Some(archetype)
                            }
                            _ => Option::None
                        }
                    }
                    Err(err) => {
                        Option::None
                    }
                }
            }
            Err(err) => {
                Option::None
            }
        }
    }

    async fn set_app_server_status(&mut self, app: AppKey, server: StarKey, status: AppServerStatus) -> Result<(), Error> {
        let (request,rx) = SupervisorDbRequest::new( SupervisorDbCommand::SetAppServerStatus(app,server,status));
        self.supervisor_db.send( request ).await;
        self.handle(rx.await)
    }

    async fn get_app_server_status(&mut self, app: &AppKey, server: &StarKey) -> AppServerStatus {
        let (request,rx) = SupervisorDbRequest::new( SupervisorDbCommand::GetAppServerStatus(app.clone(),server.clone()));
        self.supervisor_db.send( request ).await;
        match rx.await
        {
            Ok(result) => {
                match result
                {
                    Ok(ok) => {
                        match ok
                        {
                            SupervisorDbResult::AppServerStatus(status) => {
                                status
                            }
                            _ => AppServerStatus::Unknown
                        }
                    }
                    Err(err) => {
                        AppServerStatus::Unknown
                    }
                }
            }
            Err(err) => {
                AppServerStatus::Unknown
            }
        }
    }

    async fn set_app_status(&mut self, app: AppKey, status: AppStatus) -> Result<(), Error> {
        let (request,rx) = SupervisorDbRequest::new( SupervisorDbCommand::SetAppStatus(app,status));
        self.supervisor_db.send( request ).await;
        self.handle(rx.await)
    }

    async fn get_app_status(&mut self, app: &AppKey) -> AppStatus {
        let (request,rx) = SupervisorDbRequest::new( SupervisorDbCommand::GetAppStatus(app.clone()));
        self.supervisor_db.send( request ).await;
        match rx.await
        {
            Ok(result) => {
                match result
                {
                    Ok(ok) => {
                        match ok
                        {
                            SupervisorDbResult::AppStatus(status) => {
                               status
                            }
                            _ => AppStatus::Unknown
                        }
                    }
                    Err(err) => {
                        eprintln!("{}",err);
                        AppStatus::Unknown
                    }
                }
            }
            Err(err) => {
                eprintln!("{}",err);
                AppStatus::Unknown
            }
        }
    }

    async fn add_app_to_server(&mut self, app: AppKey, server: StarKey) -> Result<(), Error> {
        let (request,rx) = SupervisorDbRequest::new( SupervisorDbCommand::AddAppToServer(app, server));
        self.supervisor_db.send( request ).await;
        self.handle(rx.await)
    }

    async fn get_servers_for_app(&mut self, app: &AppKey) -> Vec<StarKey> {
        let (request,rx) = SupervisorDbRequest::new( SupervisorDbCommand::GetServersForApp(app.clone()));
        self.supervisor_db.send( request ).await;
        match rx.await
        {
            Ok(result) => {
                match result
                {
                    Ok(ok) => {
                        match ok
                        {
                            SupervisorDbResult::Servers(servers) => {
                                servers
                            }
                            _ => vec![]
                        }
                    }
                    Err(err) => {
                        eprintln!("{}",err);
                        vec![]
                    }
                }
            }
            Err(err) => {
                eprintln!("{}",err);
                vec![]
            }
        }
    }

    async fn app_sequence_next(&mut self, app: &AppKey) -> Result<u64, Error> {
        let (request,rx) = SupervisorDbRequest::new( SupervisorDbCommand::AppSequenceNext(app.clone()));
        self.supervisor_db.send( request ).await;
        match rx.await
        {
            Ok(result) => {
                match result
                {
                    Ok(ok) => {
                        match ok
                        {
                            SupervisorDbResult::AppSequenceNext(seq) => {
                                Ok(seq)
                            }
                            _ => Err("unexpected when trying to get seq".into())
                        }
                    }
                    Err(err) => {
                        eprintln!("{}",err);
                        Err("unexpected when trying to get seq".into())
                    }
                }
            }
            Err(err) => {
                eprintln!("{}",err);
                Err("unexpected when trying to get seq".into())
            }
        }
    }

    async fn set_actor_location(&mut self, location: ActorLocation) -> Result<(), Error> {
        let (request,rx) = SupervisorDbRequest::new( SupervisorDbCommand::SetActorLocation(location));
        self.supervisor_db.send( request ).await;
        self.handle(rx.await)
    }

    async fn get_actor_location(&self, actor: &ActorKey) -> Option<ActorLocation> {
        let (request,rx) = SupervisorDbRequest::new( SupervisorDbCommand::GetActorLocation(actor.clone()));
        self.supervisor_db.send( request ).await;
        match rx.await
        {
            Ok(result) => {
                match result
                {
                    Ok(ok) => {
                        match ok
                        {
                            SupervisorDbResult::ActorLocation(location ) => {
                                Option::Some(location)
                            }
                            _ => Option::None
                        }
                    }
                    Err(err) => {
                        Option::None
                    }
                }
            }
            Err(err) => {
                Option::None
            }
        }
    }
}

pub struct SupervisorDbRequest
{
    pub command: SupervisorDbCommand,
    pub tx: oneshot::Sender<Result<SupervisorDbResult,Error>>
}

impl SupervisorDbRequest
{
    pub fn new(command: SupervisorDbCommand)->(Self,oneshot::Receiver<Result<SupervisorDbResult,Error>>)
    {
        let (tx,rx) = oneshot::channel();
        (SupervisorDbRequest
         {
             command: command,
             tx: tx
         },
         rx)
    }
}

pub enum SupervisorDbCommand
{
    Close,
    AddServer(StarInfo),
    StarSelect(StarPattern),
    AddAppToServer( AppKey, StarKey),
    SetAppArchetype(AppKey,AppArchetype),
    GetAppArchetype(AppKey),
    SetAppStatus(AppKey,AppStatus),
    SetAppServerStatus(AppKey,StarKey,AppServerStatus),
    GetAppServerStatus(AppKey,StarKey),
    GetAppStatus(AppKey),
    GetServersForApp(AppKey),
    AppSequenceNext(AppKey),
    SetActorLocation(ActorLocation),
    GetActorLocation(ActorKey)
}

pub enum SupervisorDbResult
{
    Ok,
    Error(String),
    Server(Option<StarKey>),
    Servers(Vec<StarKey>),
    Supervisor(Option<StarKey>),
    AppArchetype(AppArchetype),
    AppStatus(AppStatus),
    AppServerStatus(AppServerStatus),
    AppSequenceNext(u64),
    ActorLocation(ActorLocation)
}

pub struct SupervisorDb {
    conn: Connection,
    rx: mpsc::Receiver<SupervisorDbRequest>
}

impl SupervisorDb {

    pub async fn new() -> mpsc::Sender<SupervisorDbRequest> {
        let (tx,rx) = mpsc::channel(2*1024);
        tokio::spawn( async move {
            let conn = Connection::open_in_memory();
            if conn.is_ok()
            {
                let mut db = SupervisorDb
                {
                    conn: conn.unwrap(),
                    rx: rx
                };

                db.run().await;
            }

        });

        tx
    }

    pub async fn run(&mut self)->Result<(),Error>
    {
        self.setup();

        while let Option::Some(request) = self.rx.recv().await
        {
            match request.command
            {
                SupervisorDbCommand::Close => {
                    break;
                }
                SupervisorDbCommand::AddServer(info) => {
                    let server = bincode::serialize(&info.star ).unwrap();
                    let result = self.conn.execute("INSERT INTO servers (key,kind) VALUES (?1,?2)", params![server,info.kind.to_string()]);
                    match result
                    {
                        Ok(_) => {
                            request.tx.send(Result::Ok(SupervisorDbResult::Ok) );
                        }
                        Err(e) => {
                            request.tx.send(Result::Err(e.into()) );
                        }
                    }
                }
                SupervisorDbCommand::StarSelect(pattern) => {
println!("Star SELECT ");
                    let mut statement = self.conn.prepare("SELECT key,kind FROM servers");
                    if let Result::Ok(mut statement) = statement
                    {
                        println!("got here");
                        let servers = statement.query_map( params![], |row|{
                            let key: Vec<u8> = row.get(0).unwrap();
                            if let Result::Ok(key) = bincode::deserialize::<StarKey>(key.as_slice()){
                                let kind: String = row.get(1).unwrap();
                                let kind =   StarKind::from_str(kind.as_str() ).unwrap();
                                let info = StarInfo::new(key,kind);
                                Ok(info)
                            }
                            else
                            {
                                Err(rusqlite::Error::ExecuteReturnedResults)
                            }
                        } ).unwrap();

                        let mut rtn = vec![];
                        for server in servers
                        {
                            if let Result::Ok(server) = server
                            {
                                if pattern.is_match(&server)
                                {
println!("adding server...");
                                    rtn.push(server.star );
                                }
                            }
                        }


                        request.tx.send( Result::Ok( SupervisorDbResult::Servers(rtn)));
println!("blah returning... ...");

                    }

                }
                SupervisorDbCommand::SetAppServerStatus(app, server,status ) => {
                    let server= bincode::serialize(&server).unwrap();
                    let app = bincode::serialize(&app).unwrap();

                    let transaction = self.conn.transaction().unwrap();
                    transaction.execute("REPLACE INTO apps (key) VALUES (?1)", [app.clone()]);
                    transaction.execute("REPLACE INTO apps_to_servers (app_key,server_key,status) VALUES (?1,?2,?3)", params![app.clone(), server.clone(),status.to_string()]);
                    let result = transaction.commit();

                    match result
                    {
                        Ok(_) => {
                            println!("Server set for application!");
                            request.tx.send(Result::Ok(SupervisorDbResult::Ok));
                        }

                        Err(e) => {
                            println!("ERROR setting server app: {}", e);
                            request.tx.send(Result::Err(e.into()));
                        }
                    }
                }

                SupervisorDbCommand::SetAppStatus(app, status) => {
                    let app= bincode::serialize(&app).unwrap();

                    let result = self.conn.execute("REPLACE INTO apps_status (key,status) VALUES (?1,?2)", params![app.clone(),status.to_string()]);

                    match result
                    {
                        Ok(_) => {
                            println!("app status SET");
                            request.tx.send(Result::Ok(SupervisorDbResult::Ok));
                        }
                        Err(e) => {
println!("SET APP STATUS ERROR");
                            request.tx.send(Result::Err(e.into()));
                        }
                    }

                }
                SupervisorDbCommand::GetAppStatus(app) => {
                    let app = bincode::serialize(&app ).unwrap();
                    let result = self.conn.query_row("SELECT status FROM apps_status WHERE key=?1", params![app], |row|
                        {
                            let status:String = row.get(0).unwrap();
                            let status:AppStatus = AppStatus::from_str(status.as_str()).unwrap();
                            Ok(status)
                        });
                    match result
                    {
                        Ok(status) => {
                            request.tx.send(Result::Ok(SupervisorDbResult::AppStatus(status)) );
                        }
                        Err(rusqlite::Error::QueryReturnedNoRows) => {
                            request.tx.send(Result::Ok(SupervisorDbResult::AppStatus(AppStatus::Unknown)) );
                        }
                        Err(e) => {
println!("GET APP STATUS ERROR: {}",e);
                            request.tx.send(Result::Err(e.into()) );
                        }
                    }
                }

                SupervisorDbCommand::AppSequenceNext(app) => {
                    let app = bincode::serialize(&app).unwrap();

                    let result = {
                        let transaction = self.conn.transaction().unwrap();
                        transaction.execute("UPDATE apps SET sequence=sequence+1 WHERE key=?1", [app.clone()]);
                        let result = transaction.query_row("SELECT sequence FROM apps WHERE key=?1", params![app.clone()], |row| {
                            let rtn: u64 = row.get(0).unwrap();
                            Ok(rtn)
                        });
                        let trans_result= transaction.commit();
                        if trans_result.is_err()
                        {
                            Err(trans_result.err().unwrap())
                        }
                        else {
                            result
                        }
                    };
                    match result
                    {
                        Ok(result) => {
                            println!("incremented app sequence!!!");
                            request.tx.send(Result::Ok(SupervisorDbResult::AppSequenceNext(result)));
                        }

                        Err(e) => {
                            println!("ERROR APP SEQUENCE NEXT: {}", e);
                            request.tx.send(Result::Err(e.into()));
                        }
                    }
                }

                SupervisorDbCommand::GetServersForApp(app) => {
                    println!("Get Servers For App");
                    let app = bincode::serialize(&app).unwrap();
                    let mut statement = self.conn.prepare("SELECT server_key FROM apps_to_servers WHERE app_key=?1 AND status='Ready'");
                    if let Result::Ok(mut statement) = statement
                    {
                        println!("got here");
                        let servers = statement.query_map( params![app], |row|{
                            let key: Vec<u8> = row.get(0).unwrap();
                            if let Result::Ok(key) = bincode::deserialize::<StarKey>(key.as_slice()){
                                Ok(key)
                            }
                            else
                            {
                                Err(rusqlite::Error::ExecuteReturnedResults)
                            }
                        } ).unwrap();

                        let mut rtn = vec![];
                        let servers:Vec<StarKey> = servers.into_iter().map(|r|r.unwrap()).collect();
                        request.tx.send( Result::Ok( SupervisorDbResult::Servers(rtn)));
                        println!("blah returning from GEtServers...... ...");
                    }
                }
                SupervisorDbCommand::AddAppToServer(server, app) => {
                    let app = bincode::serialize(&app).unwrap();
                    let server = bincode::serialize(&server).unwrap();

                    let result = self.conn.execute("REPLACE INTO apps_to_servers (app_key,server_key) VALUES (?1,?2)", params![app,server]);
                    match result
                    {
                        Ok(_) => {
                            request.tx.send(Result::Ok(SupervisorDbResult::Ok));
                        }
                        Err(e) => {
                            request.tx.send(Result::Ok(SupervisorDbResult::Error("AddApplicationToServer failed".into())));
                        }
                    }
                }
                SupervisorDbCommand::SetAppArchetype(app,archetype) => {
                    let app = bincode::serialize(&app).unwrap();
                    let archetype = bincode::serialize(&archetype).unwrap();

                    let result = self.conn.execute("REPLACE INTO apps (key,archetype) VALUES (?1,?2)", params![app,archetype]);
                    match result
                    {
                        Ok(_) => {
                            request.tx.send(Result::Ok(SupervisorDbResult::Ok));
                        }
                        Err(e) => {
                            request.tx.send(Result::Ok(SupervisorDbResult::Error("AddApplicationToServer failed".into())));
                        }
                    }
                }
                SupervisorDbCommand::GetAppArchetype(app) => {
                    let app = bincode::serialize(&app).unwrap();
                    let result = self.conn.query_row("SELECT archetype FROM apps WHERE key=?1", params![app], |row|
                        {
                            let archetype: Vec<u8>= row.get(0).unwrap();
                            if let Result::Ok(archetype) = bincode::deserialize::<AppArchetype>(archetype.as_slice()) {
                                Ok(archetype)
                            }
                            else
                            {
                                Err(rusqlite::Error::ExecuteReturnedResults)
                            }
                        });
                    match result
                    {
                        Ok(archetype) => {
                            request.tx.send(Ok(SupervisorDbResult::AppArchetype(archetype)));
                        }
                        Err(_) => {
                            request.tx.send(Ok(SupervisorDbResult::Error("could not find archetype".to_string())));
                        }
                    }
                }
                SupervisorDbCommand::GetAppServerStatus(app, server) => {
                    let app = bincode::serialize(&app).unwrap();
                    let server = bincode::serialize(&server).unwrap();
                    let result = self.conn.query_row("SELECT status FROM apps_to_servers WHERE app_key=?1 AND server_key=?2", params![app,server], |row|
                        {
                            let status: Vec<u8>= row.get(0).unwrap();
                            if let Result::Ok(status) = bincode::deserialize::<AppServerStatus>(status.as_slice()) {
                                Ok(status)
                            }
                            else
                            {
                                Err(rusqlite::Error::ExecuteReturnedResults)
                            }
                        });
                    match result
                    {
                        Ok(status) => {
                            request.tx.send(Ok(SupervisorDbResult::AppServerStatus(status)));
                        }
                        Err(_) => {
                            request.tx.send(Ok(SupervisorDbResult::AppServerStatus(AppServerStatus::Unknown)));
                        }
                    }
                }
                SupervisorDbCommand::SetActorLocation(loc) => {
                    let actor = bincode::serialize(&loc.actor ).unwrap();
                    let location = bincode::serialize(&loc).unwrap();

                    let result = self.conn.execute("REPLACE INTO actors (key,location) VALUES (?1,?2)", params![actor,location]);
                    match result
                    {
                        Ok(_) => {
                            request.tx.send(Result::Ok(SupervisorDbResult::Ok));
                        }
                        Err(e) => {
                            request.tx.send(Result::Ok(SupervisorDbResult::Error("SetActorLocation failed".into())));
                        }
                    }
                }
                SupervisorDbCommand::GetActorLocation(actor) => {
                    let actor= bincode::serialize(&actor).unwrap();
                    let result = self.conn.query_row("SELECT location FROM actors WHERE key=?1", params![actor], |row|
                        {
                            let location : Vec<u8>= row.get(0).unwrap();
                            if let Result::Ok(location) = bincode::deserialize::<ActorLocation>(location.as_slice()) {
                                Ok(location)
                            }
                            else
                            {
                                Err(rusqlite::Error::ExecuteReturnedResults)
                            }
                        });
                    match result
                    {
                        Ok(location) => {
                            request.tx.send(Ok(SupervisorDbResult::ActorLocation(location)));
                        }
                        Err(_) => {
                            request.tx.send(Ok(SupervisorDbResult::Error("could not find actor location".to_string())));
                        }
                    }
                }

            }

        }

        Ok(())
    }

    pub fn setup(&mut self)
    {
        let servers= r#"
       CREATE TABLE IF NOT EXISTS servers(
	      key BLOB PRIMARY KEY,
	      kind TEXT NOT NULL
        );"#;

       let apps = r#"CREATE TABLE IF NOT EXISTS apps (
         key BLOB PRIMARY KEY,
         archetype BLOB,
         sequence INTEGER DEFAULT 0
        );"#;

        let apps_status = r#"CREATE TABLE IF NOT EXISTS apps_status (
         key BLOB PRIMARY KEY,
         status TEXT NOT NULL
        );"#;


        let actors = r#"CREATE TABLE IF NOT EXISTS actors (
         key BLOB PRIMARY KEY,
         location BLOB NOT NULL
        );"#;

        let apps_to_servers = r#"CREATE TABLE IF NOT EXISTS apps_to_servers
        (
           server_key BLOB,
           app_key BLOB,
           status TEXT NOT NULL DEFAULT 'Unknown',
           PRIMARY KEY (server_key, app_key),
           FOREIGN KEY (server_key) REFERENCES servers (key),
           FOREIGN KEY (app_key) REFERENCES apps (key)
        );"#;

        {
            let transaction = self.conn.transaction().unwrap();
            transaction.execute(servers, []).unwrap();
            transaction.execute(apps, []).unwrap();
            transaction.execute(apps_status, []).unwrap();
            transaction.execute(actors, []).unwrap();
            transaction.execute(apps_to_servers, []).unwrap();
            transaction.commit();
        }

    }

}