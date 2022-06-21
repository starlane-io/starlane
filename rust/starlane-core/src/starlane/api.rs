use std::sync::Arc;
use mesh_portal::error::MsgErr;
use mesh_portal::version::latest::command::common::StateSrc;
use mesh_portal::version::latest::id::{Point, Port, TargetLayer};
use mesh_portal::version::latest::messaging::{Agent, Response};
use mesh_portal_versions::version::v0_0_1::wave::{AsyncMessenger, AsyncTransmitterWithAgent};
use mesh_portal_versions::version::v0_0_1::sys::ParticleRecord;
use cosmic_portal_cli::Cli;
use cosmic_portal_cli::CliSession;

#[derive(Clone)]
pub struct StarlaneApi {
    messenger: AsyncTransmitterWithAgent
}

impl StarlaneApi {
    pub fn new(messenger: AsyncTransmitterWithAgent) -> Self {
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

    pub fn messenger(&self) -> &AsyncTransmitterWithAgent {
        &self.messenger
    }

    pub fn messenger_from_port( &self, port: Port ) -> AsyncTransmitterWithAgent {
        self.messenger.with_from(port)
    }

    pub fn cli(&self) -> Cli {
        let messenger = self.messenger.clone().with_from( self.messenger.from.with_layer(TargetLayer::Core));
        Cli::new(messenger)
    }
}

