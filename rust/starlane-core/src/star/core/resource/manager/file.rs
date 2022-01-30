use std::convert::TryInto;
use std::sync::Arc;

use clap::{App, AppSettings};
use yaml_rust::Yaml;

use crate::artifact::ArtifactRef;
use crate::error::Error;
use crate::resource::{ArtifactKind, ResourceType, ResourceAssign, AssignResourceStateSrc};
use crate::star::core::resource::manager::ResourceManager;
use crate::star::core::resource::state::StateStore;
use crate::star::StarSkel;
use crate::watch::{Notification, Change, Topic, WatchSelector, Property};
use crate::message::delivery::Delivery;
use crate::html::html_error_code;
use crate::frame::{StarMessagePayload, StarMessage};

use std::str::FromStr;
use crate::mesh::serde::id::Address;
use mesh_portal_api::message::Message;
use mesh_portal_serde::version::latest::command::common::StateSrc;
use mesh_portal_serde::version::latest::id::Address;
use mesh_portal_serde::version::latest::messaging::Request;
use mesh_portal_serde::version::v0_0_1::generic::entity::request::ReqEntity;
use mesh_portal_serde::version::v0_0_1::generic::payload::Payload;
use crate::mesh::serde::resource::command::common::StateSrc;
use crate::mesh::Request;

#[derive(Debug)]
pub struct FileManager {
    skel: StarSkel,
    store: StateStore,
}

impl FileManager {
    pub fn new(skel: StarSkel) -> Self {
        FileManager {
            skel: skel.clone(),
            store: StateStore::new(skel),
        }
    }
}

#[async_trait]
impl ResourceManager for FileManager {
    async fn assign(
        &self,
        assign: ResourceAssign,
    ) -> Result<(), Error> {
        let state = match assign.state {
            StateSrc::StatefulDirect(data) => data,
            StateSrc::Stateless => return Err("File cannot be stateless".into()),
            _ => {
                return Err("File must specify Direct state".into() )
            }
        };

        self.store.put(assign.stub.address.clone(), state.clone() ).await?;

        let selector = WatchSelector{
            topic: Topic::Resource(assign.stub.address),
            property: Property::State
        };

        self.skel.watch_api.fire( Notification::new(selector, Change::State(state) ));

        Ok(())
    }

    async fn has(&self, key: Address) -> bool {
        match self.store.has(key).await {
            Ok(v) => v,
            Err(_) => false,
        }
    }


    fn resource_type(&self) -> ResourceType {
        ResourceType::File
    }

    fn handle_request(&self, delivery: Delivery<Request>) {
        unimplemented!();
/*        match &delivery.item {
            Message::Request(request) => {
                match &request.entity {
                    ReqEntity::Rc(_) => {}
                    ReqEntity::Msg(_) => {}
                    ReqEntity::Http(http) => {
                        unimplemented!()
                        /*
                        let state = self.store.get(key).await?.ok_or("expected state to be in the store")?;
                        let content = state.get("content").ok_or("expected file to have content")?.clone();
                        let mut response = HttpResponse::new();
                        response.status = 200;
                        response.body = Option::Some(content);
                        delivery.reply(Reply::HttpResponse(response));

                         */
                    }
                }
            }
            Message::Response(response) => {}
        }

 */
    }
}


pub struct FileSystemManager {
    skel: StarSkel,
    store: StateStore,
}

impl FileSystemManager {
    pub async fn new( skel: StarSkel ) -> Self {

        FileSystemManager {
            skel: skel.clone(),
            store: StateStore::new(skel),
        }
    }
}

#[async_trait]
impl ResourceManager for FileSystemManager {
    fn resource_type(&self) -> ResourceType {
        ResourceType::FileSystem
    }

    async fn assign(
        &self,
        assign: ResourceAssign,
    ) -> Result<(), Error> {
        match assign.state {
            StateSrc::Stateless => {}
            _ => {
                return Err("must be stateless or empty create args".into());
            }
        };

        Ok(())
    }

    async fn has(&self, key: Address) -> bool {
        match self.store.has(key).await {
            Ok(v) => v,
            Err(_) => false,
        }
    }

    fn handle_request(&self, delivery: Delivery<Request>)  {
        unimplemented!()
        /*
        let record = self.skel.resource_locator_api.locate(key.into()).await?;

        let filepath = if delivery.entity.payload.path.ends_with("/") {
            format!("{}:{}index.html", record.stub.address.to_string(),delivery.entity.payload.path )
        } else {
            format!("{}:{}", record.stub.address.to_string(),delivery.entity.payload.path )
        };

        eprintln!("FILEPATH: {}", filepath );
        let filepath = ResourcePath::from_str(filepath.as_str())?;
        let mut message = delivery.entity.clone();
        message.to = filepath.into();
        let mut star_message:StarMessage = delivery.into();
        star_message.payload = StarMessagePayload::MessagePayload(MessagePayload::HttpRequest(message));
        self.skel.router_api.route(star_message);
        Ok(())
         */
    }



}
