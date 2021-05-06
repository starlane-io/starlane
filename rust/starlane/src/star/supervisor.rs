use async_trait::async_trait;
use core::option::Option;
use core::option::Option::{None, Some};
use core::result::Result;
use core::result::Result::{Err, Ok};
use crate::star::{StarData,StarInfo, SupervisorManagerBacking, StarManager, StarManagerCommand, StarCommand, StarKey};
use crate::frame::{StarMessagePayload, StarMessage, Frame, AppNotifyCreated, ActorLookup};
use crate::error::Error;
use std::collections::HashMap;
use crate::actor::{ActorKey, ActorLocation};
use crate::app::{AppLocation, Application};
use crate::keys::AppKey;
use crate::message::{ProtoMessage, MessageExpect};
use crate::logger::{Flag, StarFlag, Log, StarLog, StarLogPayload};
use tokio::sync::mpsc::error::SendError;

pub enum SupervisorCommand
{
    Pledge
}

pub struct SupervisorManager
{
    data: StarData,
    backing: Box<dyn SupervisorManagerBacking>
}

impl SupervisorManager
{
    pub fn new(data: StarData) ->Self
    {
        SupervisorManager{
            data: data.clone(),
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
        proto.payload = StarMessagePayload::Pledge(self.data.info.kind.clone());
        proto.expect = MessageExpect::RetryUntilOk;
        let rx = proto.get_ok_result().await;
        self.data.star_tx.send(StarCommand::SendProtoMessage(proto)).await;

        if self.data.flags.check(Flag::Star(StarFlag::DiagnosePledge))
        {
            self.data.logger.log( Log::Star( StarLog::new( &self.data.info, StarLogPayload::PledgeSent )));
            let mut data = self.data.clone();
            tokio::spawn(async move {
                let payload = rx.await;
                if let Ok(StarMessagePayload::Ok) = payload
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
        let mut proto = message.reply(StarMessagePayload::Ok);
        let result = self.data.star_tx.send(StarCommand::SendProtoMessage(proto)).await;
        self.unwrap(result);
    }

    pub async fn reply_error(&self, mut message: StarMessage, error_message: String )
    {
        message.reply(StarMessagePayload::Error(error_message.to_string()));
        let result = self.data.star_tx.send(StarCommand::Frame(Frame::StarMessage(message))).await;
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
                      if self.data.flags.check( Flag::Star(StarFlag::DiagnosePledge )) {
                          self.data.logger.log( Log::Star(StarLog::new(&self.data.info, StarLogPayload::PledgeRecv )));
                      }
                  }
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
    data: StarData,
    servers: Vec<StarKey>,
    server_select_index: usize,
    applications: HashMap<AppKey,Box<dyn Application>>,
    name_to_entity: HashMap<String, ActorKey>,
    entity_location: HashMap<ActorKey, ActorLocation>
}

impl SupervisorManagerBackingDefault
{
    pub fn new(data: StarData ) ->Self
    {
        SupervisorManagerBackingDefault {
            data: data,
            servers: vec![],
            server_select_index: 0,
            applications: HashMap::new(),
            name_to_entity: HashMap::new(),
            entity_location: HashMap::new(),
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

    fn add_application(&mut self, app: AppKey, application: Box<dyn Application>) {
        self.applications.insert(app, application);
    }

    fn get_application(&mut self, app: AppKey) -> Option<&Box<dyn Application>> {
        self.applications.get(&app)
    }

    fn remove_application(&mut self, app: AppKey) {
        self.applications.remove(&app);
    }

    fn set_entity_name(&mut self, name: String, key: ActorKey) {
        self.name_to_entity.insert(name, key );
    }

    fn set_entity_location(&mut self, entity: ActorKey, location: ActorLocation) {
        self.entity_location.insert(entity, location );
    }

    fn get_entity_location(&self, lookup: &ActorLookup) -> Option<&ActorLocation> {
        match lookup
        {
            ActorLookup::Key(key) => {
                return self.entity_location.get(key)
            }
        }
    }
}
