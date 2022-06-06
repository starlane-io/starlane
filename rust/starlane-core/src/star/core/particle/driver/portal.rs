use std::convert::TryInto;
use std::sync::Arc;

use clap::{App, AppSettings};
use yaml_rust::Yaml;

use crate::artifact::ArtifactRef;
use crate::error::Error;
use crate::particle::{ArtifactSubKind, KindBase, Assign};
use crate::star::core::particle::driver::ParticleCoreDriver;
use crate::star::core::particle::state::StateStore;
use crate::star::StarSkel;
use crate::watch::{Change, Notification, Property, Topic, WatchSelector};
use crate::message::delivery::Delivery;
use crate::frame::{StarMessage, StarMessagePayload};

use std::str::FromStr;
use std::sync::atomic::AtomicU32;
use mesh_portal::version::latest::command::common::StateSrc;
use mesh_portal::version::latest::id::Point;
use mesh_portal::version::latest::messaging::{Request, Response};
use mesh_portal_tcp_server::PortalServer;

#[derive(Debug)]
pub struct PortalCoreDriver {
    skel: StarSkel,
    store: StateStore,
}

impl PortalCoreDriver {
    pub fn new(skel: StarSkel) -> Self {
        PortalCoreDriver {
            skel: skel.clone(),
            store: StateStore::new(skel),
        }
    }
}

#[async_trait]
impl ParticleCoreDriver for PortalCoreDriver {
    async fn assign(
        &mut self,
        assign: Assign,
    ) -> Result<(), Error> {
        let state = match assign.state {
            StateSrc::Payload(data) => data,
            StateSrc::None => return Err("File cannot be stateless".into()),
            _ => {
                return Err("File must specify Direct state".into() )
            }
        };

        self.store.put(assign.config.stub.point.clone(), state.clone() ).await?;

        let selector = WatchSelector{
            topic: Topic::Point(assign.config.stub.point),
            property: Property::State
        };

        self.skel.watch_api.fire( Notification::new(selector, Change::State(state) ));

        Ok(())
    }


    fn kind(&self) -> KindBase {
        KindBase::File
    }

}



fn test_logger(message: &str) {
    println!("{}", message);
}

