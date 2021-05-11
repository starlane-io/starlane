use core::option::Option;
use core::option::Option::{None, Some};
use core::result::Result;
use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use tokio::sync::mpsc::error::SendError;

use crate::actor::{ActorKey, ActorLocation};
use crate::app::{AppMeta, Application, AppLocation, AppStatus, AppReadyStatus, AppPanicReason, AppArchetype, InitData, AppCreateResult, App};
use crate::error::Error;
use crate::frame::{ActorLookup, AppNotifyCreated, AssignMessage, Frame, Reply, SpaceMessage, SpacePayload, StarMessage, StarMessagePayload, AppMessage, ServerAppPayload, SpaceReply, StarMessageCentral, SimpleReply, SupervisorPayload, StarMessageSupervisor, ServerPayload};
use crate::keys::{AppKey, UserKey};
use crate::logger::{Flag, Log, StarFlag, StarLog, StarLogPayload};
use crate::message::{MessageExpect, ProtoMessage, MessageExpectWait};
use crate::star::{StarCommand, StarSkel, StarInfo, StarKey, StarManager, StarManagerCommand};
use tokio::sync::oneshot::Receiver;
use tokio::sync::oneshot::error::RecvError;
use crate::star::supervisor::SupervisorCommand::AppAssign;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

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

pub struct SupervisorManager
{
    skel: StarSkel,
    backing: Box<dyn SupervisorManagerBacking>
}

impl SupervisorManager
{
    pub fn new(data: StarSkel) ->Self
    {
        SupervisorManager{
            skel: data.clone(),
            backing: Box::new(SupervisorManagerBackingDefault::new(data)),
        }
    }
}

impl SupervisorManager
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
        message.reply(StarMessagePayload::Reply(SimpleReply::Error(error_message.to_string())));
        let result = self.skel.star_tx.send(StarCommand::Frame(Frame::StarMessage(message))).await;
        self.unwrap(result);
    }

}

#[async_trait]
impl StarManager for SupervisorManager
{
    async fn handle(&mut self, command: StarManagerCommand) {
        match command
        {
            StarManagerCommand::Init => {
                self.pledge().await;
            }
            StarManagerCommand::SupervisorCommand(supervisor_command) => {
                match supervisor_command
                {
                    SupervisorCommand::Pledge => {
                        self.pledge().await;
                    }
                    SupervisorCommand::SetAppStatus(set_app_status) => {
                        self.backing.set_app_status(set_app_status.app.clone(), set_app_status.status.clone());
                    }
                    SupervisorCommand::AppAssign(app) => {
                        let servers = self.backing.servers();
                        if !servers.is_empty()
                        {
                            for server in servers
                            {
                                self.backing.set_app_server_status(app.key.clone(), server.clone(), AppServerStatus::Assigning );
                                let mut proto = ProtoMessage::new();
                                proto.to(server.clone());
                                proto.payload = StarMessagePayload::Space(SpaceMessage{
                                    sub_space: app.key.sub_space.clone(),
                                    user: UserKey::hyperuser(),
                                    payload: SpacePayload::Server(ServerPayload::AppAssign(app.meta()))
                                });
                                let result = proto.get_ok_result().await;
                                self.skel.star_tx.send( StarCommand::SendProtoMessage(proto)).await;
                                let manager_tx = self.skel.manager_tx.clone();
                                let app = app.clone();
                                let server= server.clone();
                                tokio::spawn( async move {
                                    match result.await
                                    {
                                        Ok(_) => {
                                            manager_tx.send(StarManagerCommand::SupervisorCommand(SupervisorCommand::SetAppServerStatus(SetAppServerStatus{
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
println!("SetAppServerStatus {}", set_status.status);
                        self.backing.set_app_server_status(set_status.app.clone(),set_status.server.clone(), set_status.status.clone() );
                        if self.backing.get_app_status(&set_status.app) == AppStatus::Pending
                        {
                            self.backing.set_app_status(set_status.app.clone(),  AppStatus::Launching );
                            self.skel.manager_tx.send(StarManagerCommand::SupervisorCommand(SupervisorCommand::AppLaunch(set_status.app.clone()))).await;
                        }
                    }
                    SupervisorCommand::AppLaunch(app_key) => {
println!("SUpervisor AppLaunch");
                        let archetype = self.backing.get_application(&app_key).cloned();
                        let server = self.backing.select_server(&app_key);

                        if archetype.is_none()
                        {
                            eprintln!("cannot find archetype for app: {}",app_key);
                        }

                        if server.is_none()
                        {
                            eprintln!("cannot select a server for app: {}",app_key);
                        }

                        if let Option::Some(archetype) = archetype
                        {
                            if let Option::Some(server) = server
                            {
                                let app = App {
                                    key: app_key.clone(),
                                    archetype: archetype.clone()
                                };
                                let mut proto = ProtoMessage::new();
                                proto.to(server.clone());
                                proto.payload = StarMessagePayload::Space(SpaceMessage {
                                    sub_space: app.key.sub_space.clone(),
                                    user: UserKey::hyperuser(),
                                    payload: SpacePayload::Server(ServerPayload::AppLaunch(app))
                                });
                                let result = proto.get_ok_result().await;
                                self.skel.star_tx.send(StarCommand::SendProtoMessage(proto)).await;

                                let manager_tx = self.skel.manager_tx.clone();
                                tokio::spawn(async move {
                                    match result.await
                                    {
                                        Ok(_) => {
                                            manager_tx.send(StarManagerCommand::SupervisorCommand(SupervisorCommand::SetAppStatus(SetAppStatus {
                                                app: app_key,
                                                status: AppStatus::Ready(AppReadyStatus::Nominal)
                                            }))).await;
                                            println!("~~~ >   app status set to READY!!! ");
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
            }
            StarManagerCommand::StarMessage( star_message) =>
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
println!("Supervisor: Received App Create");
                                        let app_key = AppKey::new(space_message.sub_space.clone());
                                        self.backing.add_application(app_key.clone(), archetype.clone() );
                                        self.backing.set_app_status(app_key.clone(), AppStatus::Pending);
                                        let proto = star_message.reply( StarMessagePayload::Reply(SimpleReply::Ok(Reply::Empty)));
                                        self.skel.star_tx.send(StarCommand::SendProtoMessage(proto)).await;
                                        let app = App{
                                            key: app_key,
                                            archetype: archetype.clone()
                                        };
                                        self.skel.manager_tx.send( StarManagerCommand::SupervisorCommand(SupervisorCommand::AppAssign(app))).await;
                                    }
                                    SupervisorPayload::AppSequenceRequest(app_key) => {
                                        let index = self.backing.app_sequence_next(app_key);
                                        let reply = star_message.reply(StarMessagePayload::Reply(SimpleReply::Ok(Reply::Seq(index))));
                                        self.skel.star_tx.send(StarCommand::SendProtoMessage(reply)).await;
                                    }
                                    SupervisorPayload::ActorRegister(_) => {}
                                    SupervisorPayload::ActorUnRegister(_) => {}
                                    SupervisorPayload::ActorStatus(_) => {}
                                }
                            }
                            _ => {}
                        }
                    }
                    StarMessagePayload::Supervisor(star_message_supervisor)=> {
                            match star_message_supervisor
                            {
                                StarMessageSupervisor::Pledge(kind) => {
                                    self.backing.add_server(star_message.from.clone());
                                    self.reply_ok(star_message.clone()).await;
                                    if self.skel.flags.check( Flag::Star(StarFlag::DiagnosePledge )) {

                                        self.skel.logger.log( Log::Star(StarLog::new(&self.skel.info, StarLogPayload::PledgeRecv )));
                                    }
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



impl SupervisorManager
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

impl SupervisorManagerBacking for SupervisorManagerBackingDefault
{
    fn add_server(&mut self, server: StarKey) {
        self.servers.push(server);
    }

    fn remove_server(&mut self, server: &StarKey) {
        self.servers.retain(|star| star != server );
    }

    fn select_server(&mut self, app: &AppKey ) -> Option<StarKey> {
        if self.servers.len() == 0
        {
            return Option::None;
        }
        self.server_select_index = &self.server_select_index +1;
        let server = self.servers.get( &self.server_select_index % self.servers.len() ).unwrap();
        Option::Some(server.clone())
    }

    fn servers(&mut self) -> Vec<StarKey> {
        self.servers.clone()
    }

    fn add_application(&mut self, app: AppKey, data: AppArchetype) {
        self.applications.insert(app, data );
    }

    fn get_application(&mut self, app: &AppKey) -> Option<&AppArchetype> {
        self.applications.get(&app )
    }

    fn set_app_server_status(&mut self, app: AppKey, server: StarKey, status: AppServerStatus) {
println!("AppServerStatus: {}",status);
        self.app_server_status.insert( (app,server), status );
    }

    fn get_app_server_status(&mut self, app: &AppKey, server: &StarKey) -> AppServerStatus {
        if let Option::Some(status) = self.app_server_status.get( &(app.clone(),server.clone()) )
        {
            status.clone()
        }
        else
        {
            AppServerStatus::Unknown
        }
    }

    fn set_app_status(&mut self, app: AppKey, status: AppStatus){
        self.app_status.insert( app, status );
    }

    fn get_app_status(&mut self, app: &AppKey) -> AppStatus {
        match self.app_status.get( &app,)
        {
            Some(status) => status.clone(),
            None => AppStatus::Unknown
        }
    }

    fn add_app_to_server(&mut self, app: AppKey, server: StarKey) {
        if !self.app_to_servers.contains_key( &app )
        {
            let mut servers = HashSet::new();
            self.app_to_servers.insert( app.clone(), servers );
        }

        let mut servers = self.app_to_servers.get_mut(&app).unwrap();
        servers.insert(server);
    }


    fn get_servers_for_app(&mut self, app: &AppKey) -> Vec<StarKey> {
        let servers = self.app_to_servers.get(&app).unwrap();
        servers.clone().iter().map(|i|i.clone()).collect()
    }

    fn app_sequence_next(&mut self, app: &AppKey) -> u64 {
        if !self.app_sequence.contains_key(&app)
        {
            self.app_sequence.insert( app.clone(), AtomicU64::new(0) );
        }
        let atomic = self.app_sequence.get_mut(app).unwrap();
        atomic.fetch_add(1, Ordering::Relaxed )
    }

    fn remove_application(&mut self, app: &AppKey) {
        self.applications.remove(app);
    }

    fn set_actor_location(&mut self, entity: ActorKey, location: ActorLocation) {
        self.actor_location.insert(entity, location );
    }

    fn get_actor_location(&self, lookup: &ActorLookup) -> Option<&ActorLocation> {
        match lookup
        {
            ActorLookup::Key(key) => {
                return self.actor_location.get(key)
            }
        }
    }
}



pub trait SupervisorManagerBacking: Send+Sync
{
    fn add_server( &mut self, server: StarKey );
    fn remove_server( &mut self, server: &StarKey );
    fn select_server(&mut self, app:  &AppKey ) -> Option<StarKey>;
    fn servers(&mut self) -> Vec<StarKey>;

    fn add_application(&mut self, app: AppKey, archetype: AppArchetype);
    fn get_application(&mut self, app: &AppKey ) -> Option<&AppArchetype>;
    fn set_app_server_status(&mut self, app: AppKey, server: StarKey, status: AppServerStatus);
    fn get_app_server_status(&mut self, app: &AppKey, server: &StarKey) ->AppServerStatus;
    fn set_app_status(&mut self, app: AppKey, status: AppStatus );
    fn get_app_status(&mut self, app: &AppKey) -> AppStatus;
    fn add_app_to_server(&mut self, app: AppKey, server: StarKey);
    fn get_servers_for_app(&mut self, app: &AppKey ) -> Vec<StarKey>;

    fn app_sequence_next(&mut self, app: &AppKey ) -> u64;

    fn remove_application(&mut self, app: &AppKey );

    fn set_actor_location(&mut self, actor: ActorKey, location: ActorLocation);
    fn get_actor_location(&self, lookup: &ActorLookup) -> Option<&ActorLocation>;
}


#[derive(Clone)]
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
