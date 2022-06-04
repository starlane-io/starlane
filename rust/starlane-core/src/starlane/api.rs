use std::sync::Arc;
use mesh_portal::error::MsgErr;
use mesh_portal::version::latest::command::common::StateSrc;
use mesh_portal::version::latest::id::{Point, Port};
use mesh_portal::version::latest::messaging::{Agent, Response};
use mesh_portal_versions::version::v0_0_1::messaging::{AsyncMessenger, AsyncMessengerAgent};
use crate::particle::ParticleRecord;

#[derive(Clone)]
pub struct StarlaneApi {
    messenger: AsyncMessengerAgent
}

impl StarlaneApi {
    pub fn new( messenger: AsyncMessengerAgent ) -> Self {
        Self {
            messenger
        }
    }
}

impl StarlaneApi {
    pub async fn get_state( &self, point: Point ) -> Response {
        unimplemented!()
    }

    pub async fn set_state( &self, point: Point, state: StateSrc ) -> Response {
        unimplemented!()
    }

    pub fn messenger(&self) -> &AsyncMessengerAgent {
        &self.messenger
    }

    pub fn messenger_from_port( &self, port: Port ) -> AsyncMessengerAgent {
        self.messenger.with_from(port)
    }
}

