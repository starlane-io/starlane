use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use async_trait::async_trait;
use core::option::Option;
use core::option::Option::{None, Some};
use core::result::Result;
use core::result::Result::{Err, Ok};
use starlane::actor::{ActorKey, ActorLocation};
use starlane::app::{AppKey, Application, AppLocation};
use starlane::error::Error;
use starlane::frame::{ActorLookup, ApplicationNotifyReady, Frame, StarMessage, StarMessagePayload};
use starlane::star::{StarCommand, StarInfo, StarKey, StarManager, StarManagerCommand, SupervisorManagerBacking};
use crate::star::{StarInfo, SupervisorManagerBacking, StarManager, StarManagerCommand, StarCommand, StarKey};
use crate::frame::{StarMessagePayload, StarMessage, Frame, AppNotifyCreated, ActorLookup};
use crate::error::Error;
use crate::app::{Application, AppLocation, AppKey};
use std::collections::HashMap;
use crate::actor::{ActorKey, ActorLocation};

pub enum SupervisorCommand
{
    PledgeToCentral
}


pub struct SupervisorManager
{
    info: StarInfo,
    backing: Box<dyn SupervisorManagerBacking>
}

impl SupervisorManager
{
    pub fn new(info: StarInfo)->Self
    {
        SupervisorManager{
            info: info.clone(),
            backing: Box::new(SupervisorManagerBackingDefault::new(info)),
        }
    }
}

#[async_trait]
impl StarManager for SupervisorManager
{
    async fn handle(&mut self, command: StarManagerCommand) -> Result<(), Error> {
        match command
        {
            StarManagerCommand::Init => {
               let payload = StarMessagePayload::SupervisorPledgeToCentral;
               let message = StarMessage::new(self.info.sequence.next(), self.info.star_key.clone(), StarKey::central(), payload );
               let command = StarCommand::Frame(Frame::StarMessage(message));
               self.info.command_tx.send( command ).await;
               Ok(())
            }
            StarManagerCommand::Frame(frame) => {
                match frame {
                    Frame::StarMessage(message) => {
                        self.handle_message(message).await
                    }
                    _ => Err(format!("{} manager does not know how to handle frame: {}", self.info.kind, frame).into())
                }
            }
            StarManagerCommand::SupervisorCommand(command) => {
                if let SupervisorCommand::PledgeToCentral = command
                {
                    let message = StarMessage::new(self.info.sequence.next(), self.info.star_key.clone(), StarKey::central(), StarMessagePayload::SupervisorPledgeToCentral );
                    Ok(self.info.command_tx.send( StarCommand::Frame(Frame::StarMessage(message))).await?)
                }
                else {
                    Err(format!("{} manager does not know how to handle : ...", self.info.kind).into())
                }
            }
            StarManagerCommand::ServerCommand(_) => {
                Err(format!("{} manager does not know how to handle : {}", self.info.kind, command).into())
            }
            StarManagerCommand::ActorCommand(_) => {
                Err(format!("{} manager does not know how to handle : {}", self.info.kind, command).into())
            }
        }
    }
}


impl SupervisorManager
{
    async fn handle_message(&mut self, message: StarMessage) -> Result<(), Error> {

        let mut message = message;
        match &message.payload
        {
            StarMessagePayload::ApplicationAssign(assign) => {

                let application = Application::new(assign.app.clone(), assign.data.clone() );
                self.backing.add_application(assign.app.key.clone(), application);

                // TODO: Now we need to Launch this application in the ext
                // ext.launch_app()

                for notify in &assign.notify
                {
                    let location = AppLocation{
                        app: assign.app.key.clone(),
                        supervisor: assign.supervisor.clone()
                    };
                    let payload = StarMessagePayload::ApplicationNotifyReady(AppNotifyCreated { location: location});
                    let mut notify_app_ready = StarMessage::new(self.info.sequence.next(), self.info.star_key.clone(), notify.clone(), payload );
                    notify_app_ready.transaction = message.transaction.clone();
                    self.info.command_tx.send(StarCommand::Frame(Frame::StarMessage(notify_app_ready))).await?;
                }

                Ok(())
            }
            StarMessagePayload::ServerPledgeToSupervisor => {

                self.backing.add_server(message.from.clone());
                Ok(())
            }
            StarMessagePayload::ActorLocationReport(report) =>
            {
                    self.backing.set_entity_location(report.actor.clone(), report.clone());
                    Ok(())
            }
            StarMessagePayload::ActorLocationRequest(request) =>
                {

                    let location = self.backing.get_entity_location(&request.lookup);

                    match location
                    {
                        None => {
                            return Err(format!("cannot find entity: {}", request.lookup).into() );
                        }
                        Some(location) => {
                            let location = location.clone();
                            let payload = StarMessagePayload::ActorLocationReport(location);
                            message.reply( self.info.sequence.next(), payload );
                            self.info.command_tx.send( StarCommand::Frame(Frame::StarMessage(message))).await?;
                        }
                    }
                    Ok(())
                }
            _ => {
                Err("SupervisorCore does not handle message of this type: _".into())
            }
        }
    }
}

pub struct SupervisorManagerBackingDefault
{
    info: StarInfo,
    servers: Vec<StarKey>,
    server_select_index: usize,
    applications: HashMap<AppKey,Application>,
    name_to_entity: HashMap<String, ActorKey>,
    entity_location: HashMap<ActorKey, ActorLocation>
}

impl SupervisorManagerBackingDefault
{
    pub fn new(info: StarInfo)->Self
    {
        SupervisorManagerBackingDefault {
            info: info,
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

    fn add_application(&mut self, app: AppKey, application: Application ) {
        self.applications.insert(app, application);
    }

    fn get_application(&mut self, app: AppKey) -> Option<&Application> {
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
            ActorLookup::Name(lookup) => {

                if let Some(key) = self.name_to_entity.get(&lookup.name)
                {
                    return self.entity_location.get(key)
                }
                else {
                    Option::None
                }
            }
        }
    }
}
