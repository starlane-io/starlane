use std::convert::TryInto;
use std::sync::Arc;

use clap::{App, AppSettings};
use yaml_rust::Yaml;

use crate::artifact::ArtifactRef;
use crate::error::Error;
use crate::resource::{ArtifactKind, ResourceType, ResourceAssign, AssignResourceStateSrc};
use crate::star::core::resource::shell::Host;
use crate::star::core::resource::state::StateStore;
use crate::star::StarSkel;
use crate::watch::{Notification, Change, Topic, WatchSelector, Property};
use crate::message::delivery::Delivery;
use crate::html::html_error_code;
use crate::frame::{StarMessagePayload, StarMessage};

use std::str::FromStr;
use crate::mesh::serde::id::Address;
use mesh_portal_api::message::Message;
use mesh_portal_serde::version::v0_0_1::generic::entity::request::ReqEntity;
use mesh_portal_serde::version::v0_0_1::generic::payload::Payload;

#[derive(Debug)]
pub struct FileHost {
    skel: StarSkel,
    store: StateStore,
}

impl FileHost {
    pub async fn new(skel: StarSkel) -> Self {
        FileHost {
            skel: skel.clone(),
            store: StateStore::new(skel).await,
        }
    }
}

#[async_trait]
impl Host for FileHost {
    async fn assign(
        &self,
        assign: ResourceAssign<AssignResourceStateSrc>,
    ) -> Result<(), Error> {
        let state = match assign.state {
            AssignResourceStateSrc::Direct(data) => data,
            AssignResourceStateSrc::Stateless => return Err("File cannot be stateless".into()),
            _ => {
                return Err("File must specify Direct state".into() )
            }
        };

        let state= self.store.put(assign.stub.key.clone(), state ).await?;

        let selector = WatchSelector{
            topic: Topic::Resource(assign.stub.key),
            property: Property::State
        };

        self.skel.watch_api.fire( Notification::new(selector, Change::State(state.clone()) ));

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

    fn handle(&self,delivery: Delivery<Message>) {
        match &delivery.item {
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
    }
}


pub struct FileSystemHost {
    skel: StarSkel,
    store: StateStore,
}

impl FileSystemHost{
    pub async fn new( skel: StarSkel ) -> Self {

        FileSystemHost{
            skel: skel.clone(),
            store: StateStore::new(skel).await,
        }
    }
}

#[async_trait]
impl Host for FileSystemHost {
    fn resource_type(&self) -> ResourceType {
        ResourceType::FileSystem
    }

    async fn assign(
        &self,
        assign: ResourceAssign<AssignResourceStateSrc>,
    ) -> Result<(), Error> {
        match assign.state {
            AssignResourceStateSrc::Stateless => {}
            AssignResourceStateSrc::CreateArgs(_) => {}
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

    fn handle(&self,  delivery: Delivery<Message>)  {
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
