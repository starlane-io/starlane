use core::option::Option;
use core::option::Option::{None, Some};
use core::result::Result;
use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use tokio::sync::mpsc::error::SendError;

use crate::actor::{ActorKey, ActorLocation};
use crate::app::{AppInfo, Application, AppLocation, AppStatus, AppReadyStatus, AppPanicReason};
use crate::error::Error;
use crate::frame::{ActorLookup, AppNotifyCreated, AssignMessage, Frame, Reply, SpaceMessage, SpacePayload, StarMessage, StarMessagePayload, AppMessage, AppMessagePayload, RequestMessage, ReportMessage};
use crate::keys::AppKey;
use crate::logger::{Flag, Log, StarFlag, StarLog, StarLogPayload};
use crate::message::{MessageExpect, ProtoMessage, MessageExpectWait};
use crate::star::{StarCommand, StarSkel, StarInfo, StarKey, StarManager, StarManagerCommand};
use tokio::sync::oneshot::Receiver;
use tokio::sync::oneshot::error::RecvError;

pub enum SupervisorCommand
{
    Pledge,
    SetAppStatus(SetAppStatus)
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
        proto.payload = StarMessagePayload::Pledge(self.skel.info.kind.clone());
        proto.expect = MessageExpect::RetryUntilOk;
        let rx = proto.get_ok_result().await;
        self.skel.star_tx.send(StarCommand::SendProtoMessage(proto)).await;

        if self.skel.flags.check(Flag::Star(StarFlag::DiagnosePledge))
        {
            self.skel.logger.log( Log::Star( StarLog::new(&self.skel.info, StarLogPayload::PledgeSent )));
            let mut data = self.skel.clone();
            tokio::spawn(async move {
                let payload = rx.await;
                if let Ok(StarMessagePayload::Ok(_)) = payload
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
        let mut proto = message.reply(StarMessagePayload::Ok(Reply::Empty));
        let result = self.skel.star_tx.send(StarCommand::SendProtoMessage(proto)).await;
        self.unwrap(result);
    }

    pub async fn reply_error(&self, mut message: StarMessage, error_message: String )
    {
        message.reply(StarMessagePayload::Error(error_message.to_string()));
        let result = self.skel.star_tx.send(StarCommand::Frame(Frame::StarMessage(message))).await;
        self.unwrap(result);
    }

}

#[async_trait]
impl StarManager for SupervisorManager
{
    async fn handle(&mut self, command: StarManagerCommand)  {

        match command
        {

           StarManagerCommand::Init => {
               self.pledge().await;
           }
           StarManagerCommand::StarMessage(message)=>{
              match &message.payload
              {
                  StarMessagePayload::Pledge(kind) => {
                      self.backing.add_server(message.from.clone());
                      self.reply_ok(message).await;
                      if self.skel.flags.check( Flag::Star(StarFlag::DiagnosePledge )) {

                          self.skel.logger.log( Log::Star(StarLog::new(&self.skel.info, StarLogPayload::PledgeRecv )));
                      }
                  }
                  StarMessagePayload::Space(space_message) =>
                  {
                      match &space_message.payload
                      {
                          SpacePayload::Assign(assign) => {
                              match assign
                              {
                                  AssignMessage::App(app_assign) => {
                                      let app = app_assign.app.clone();
                                      let data = AppData{
                                          info: AppInfo{
                                              key: app_assign.app.clone(),
                                              config : app_assign.info.config.clone()
                                          },
                                          servers: HashSet::new()
                                      };
                                      self.backing.add_application(app.clone(), data );
                                      let proto = message.reply(StarMessagePayload::Ok(Reply::Empty));
                                      self.skel.star_tx.send(StarCommand::SendProtoMessage(proto)).await;

                                      if let Option::Some(server)=self.backing.select_server()
                                      {
                                          self.backing.set_app_status(app.clone(), AppStatus::Launching );
                                          let launch_app_message = space_message.with_payload(SpacePayload::App(AppMessage { app: app.clone(), payload: AppMessagePayload::Create(app_assign.info.clone()) }));
                                          let mut proto = ProtoMessage::new();
                                          proto.to = Option::Some(server);
                                          proto.payload = StarMessagePayload::Space(launch_app_message);
                                          proto.expect = MessageExpect::ReplyErrOrTimeout(MessageExpectWait::Med);
                                          let result = proto.get_ok_result().await;

                                          let manager_tx = self.skel.manager_tx.clone();
                                          tokio::spawn( async move {

                                              match result.await
                                              {
                                                  Ok(payload) => {
                                                      match payload {
                                                          StarMessagePayload::Ok(Reply::Empty) => {
                                                              manager_tx.send( StarManagerCommand::SupervisorCommand(SupervisorCommand::SetAppStatus(SetAppStatus{app: app.clone(), status: AppStatus::Ready(AppReadyStatus::Nominal)}))).await;
                                                          }
                                                          _ => {
                                                              manager_tx.send( StarManagerCommand::SupervisorCommand(SupervisorCommand::SetAppStatus(SetAppStatus{app: app.clone(), status: AppStatus::Panic( AppPanicReason::Desc("unexpected replay from server...".to_string()) )}))).await;
                                                          }
                                                      }
                                                  }
                                                  Err(error) => {
                                                      manager_tx.send( StarManagerCommand::SupervisorCommand(SupervisorCommand::SetAppStatus(SetAppStatus{app: app.clone(), status: AppStatus::Panic( AppPanicReason::Desc(error.to_string()) )}))).await;
                                                  }
                                              }

                                          } );
                                          self.skel.star_tx.send( StarCommand::SendProtoMessage(proto)).await;
                                      }
                                      else {
                                          self.backing.set_app_status(app_assign.app.clone(), AppStatus::Waiting )
                                      }

                                  }
                              }
                          }
                          SpacePayload::Request(request)=>
                          {
                              match request
                              {
                                  RequestMessage::AppSequenceRequest(app) => {
                                     let seq = self.backing.app_sequence_next(app);
                                     let proto = message.reply(StarMessagePayload::Space(space_message.with_payload(SpacePayload::Report(ReportMessage::AppSequenceResponse(seq)))));
                                     self.skel.star_tx.send(StarCommand::SendProtoMessage(proto)).await;
                                  }
                                  _ => {

                                      eprintln!("supervisor manager doesn't handle this request..." );
                                  }
                              }
                          }

                          _ => {
                              eprintln!("supervisor manager doesn't handle ..." );
                          }
                      }

                  }
                  StarMessagePayload::Ok(_)=>{}
                  what => {
                      eprintln!("supervisor manager doesn't handle {}", what )
                  }
              }
           }
           StarManagerCommand::SupervisorCommand(command) => {
                match command{
                    SupervisorCommand::Pledge => {
                        self.pledge().await;
                    }
                    SupervisorCommand::SetAppStatus(set_app_status) => {
                        self.backing.set_app_status(set_app_status.app,set_app_status.status)
                    }
                }
            }
            what => {
                eprintln!("supervisor manager doesn't handle {}", what )
            }
        }

    }
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
    applications: HashMap<AppKey,AppData>,
    actor_location: HashMap<ActorKey, ActorLocation>
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

    fn select_server(&mut self) -> Option<StarKey> {
        if self.servers.len() == 0
        {
            return Option::None;
        }
        self.server_select_index = &self.server_select_index +1;
        let server = self.servers.get( &self.server_select_index % self.servers.len() ).unwrap();
        Option::Some(server.clone())
    }

    fn add_application(&mut self, app: AppKey, data: AppData ) {
        self.applications.insert(app, data );
    }

    fn get_application(&mut self, app: &AppKey) -> Option<&AppData> {
        self.applications.get(&app )
    }

    fn set_app_status(&mut self, app: AppKey, status: AppStatus){
println!("SET APP STATUS: {}", status );
    }

    fn get_app_status(&mut self, app: &AppKey) -> AppStatus {
        AppStatus::Unknown
    }

    fn app_sequence_next(&mut self, app: &AppKey) -> u64 {
       // HACK (we will be changing the backing to SqlLite very soon)
       0
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

pub struct AppData
{
    pub info: AppInfo,
    pub servers: HashSet<StarKey>
}

impl AppData
{
    pub fn new(info: AppInfo)->Self
    {
        AppData{
            info: info,
            servers: HashSet::new()
        }
    }
}

pub trait SupervisorManagerBacking: Send+Sync
{
    fn add_server( &mut self, server: StarKey );
    fn remove_server( &mut self, server: &StarKey );
    fn select_server(&mut self) -> Option<StarKey>;

    fn add_application(&mut self, app: AppKey, app_data: AppData );
    fn get_application(&mut self, app: &AppKey ) -> Option<&AppData>;
    fn set_app_status(&mut self, app: AppKey, status: AppStatus );
    fn get_app_status(&mut self, app: &AppKey) -> AppStatus;
    fn app_sequence_next(&mut self, app: &AppKey ) -> u64;

    fn remove_application(&mut self, app: &AppKey );

    fn set_actor_location(&mut self, actor: ActorKey, location: ActorLocation);
    fn get_actor_location(&self, lookup: &ActorLookup) -> Option<&ActorLocation>;
}
