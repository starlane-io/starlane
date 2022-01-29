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
use std::sync::atomic::AtomicU32;
use crate::mesh::serde::id::Address;
use mesh_portal_api::message::Message;
use mesh_portal_api_server::PortalRequestHandler;
use mesh_portal_serde::version::v0_0_1::generic::entity::request::ReqEntity;
use mesh_portal_serde::version::v0_0_1::generic::payload::Payload;
use mesh_portal_tcp_server::PortalServer;
use crate::mesh::serde::resource::command::common::StateSrc;
use crate::mesh::Request;

#[derive(Debug)]
pub struct PortalManager {
    skel: StarSkel,
    store: StateStore,
}

impl PortalManager {
    pub fn new(skel: StarSkel) -> Self {
        PortalManager {
            skel: skel.clone(),
            store: StateStore::new(skel),
        }
    }
}

#[async_trait]
impl ResourceManager for PortalManager {
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



fn test_logger(message: &str) {
    println!("{}", message);
}

