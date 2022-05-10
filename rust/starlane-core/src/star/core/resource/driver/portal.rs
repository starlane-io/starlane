use std::convert::TryInto;
use std::sync::Arc;

use clap::{App, AppSettings};
use yaml_rust::Yaml;

use crate::artifact::ArtifactRef;
use crate::error::Error;
use crate::particle::{ArtifactSubKind, KindBase, ParticleAssign, AssignResourceStateSrc};
use crate::star::core::resource::driver::ParticleCoreDriver;
use crate::star::core::resource::state::StateStore;
use crate::star::StarSkel;
use crate::watch::{Notification, Change, Topic, WatchSelector, Property};
use crate::message::delivery::Delivery;
use crate::frame::{StarMessagePayload, StarMessage};

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
        assign: ParticleAssign,
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


    fn resource_type(&self) -> KindBase {
        KindBase::File
    }

}



fn test_logger(message: &str) {
    println!("{}", message);
}

