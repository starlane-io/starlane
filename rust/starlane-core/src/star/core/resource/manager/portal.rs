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
use mesh_portal_serde::version::latest::command::common::StateSrc;
use mesh_portal_serde::version::latest::id::Address;
use mesh_portal_serde::version::latest::messaging::{Request, Response};
use mesh_portal_tcp_server::PortalServer;

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
        &mut self,
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


    fn resource_type(&self) -> ResourceType {
        ResourceType::File
    }

}



fn test_logger(message: &str) {
    println!("{}", message);
}

