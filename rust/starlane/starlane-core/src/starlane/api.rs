use cosmic_portal_cli::Cli;
use cosmic_portal_cli::CliSession;
use cosmic_universe::hyper::ParticleRecord;
use cosmic_universe::wave::{AsyncTransmitterWithAgent, Transmitter};
use mesh_portal::error::MsgErr;
use mesh_portal::version::latest::command::common::StateSrc;
use mesh_portal::version::latest::id::{Point, Port, TargetLayer};
use mesh_portal::version::latest::messaging::{Agent, RespShell};
use std::sync::Arc;

#[derive(Clone)]
pub struct StarlaneApi {
    transmitter: AsyncTransmitterWithAgent,
}

impl StarlaneApi {
    pub fn new(messenger: AsyncTransmitterWithAgent) -> Self {
        Self {
            transmitter: messenger,
        }
    }
}

impl StarlaneApi {
    pub async fn get_state(&self, point: Point) -> RespShell {
        unimplemented!()
    }

    pub async fn set_state(&self, point: Point, state: StateSrc) -> RespShell {
        unimplemented!()
    }

    pub fn transmitter(&self) -> &AsyncTransmitterWithAgent {
        &self.transmitter
    }

    pub fn messenger_from_port(&self, port: Port) -> AsyncTransmitterWithAgent {
        self.transmitter.with_from(port)
    }

    pub fn cli(&self) -> Cli {
        let messenger = self
            .transmitter
            .clone()
            .with_from(self.transmitter.from.with_layer(TargetLayer::Core));
        Cli::new(messenger)
    }
}
