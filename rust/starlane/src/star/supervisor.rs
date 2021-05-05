use async_trait::async_trait;
use core::option::Option;
use core::option::Option::{None, Some};
use core::result::Result;
use core::result::Result::{Err, Ok};
use crate::star::{StarInfo, SupervisorManagerBacking, StarManager, StarManagerCommand, StarCommand, StarKey};
use crate::frame::{StarMessagePayload, StarMessage, Frame, AppNotifyCreated, ActorLookup};
use crate::error::Error;
use std::collections::HashMap;
use crate::actor::{ActorKey, ActorLocation};
use crate::app::{AppLocation, Application};
use crate::keys::AppKey;

pub enum SupervisorCommand
{
    Pledge
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
    async fn handle(&mut self, command: StarManagerCommand)  {

    }
}


impl SupervisorManager
{
    async fn handle_message(&mut self, message: StarMessage) {

    }
}

pub struct SupervisorManagerBackingDefault
{
    info: StarInfo,
    servers: Vec<StarKey>,
    server_select_index: usize,
    applications: HashMap<AppKey,Box<dyn Application>>,
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
