use std::fmt;

use tokio::sync::oneshot;

use crate::error::Error;
use crate::frame::{StarMessage, StarMessagePayload};
use crate::lane::LaneWrapper;
use crate::star::{CoreRequest, StarCommand, StarKind, StarSkel};
use crate::star::variant::central::CentralVariant;
use crate::star::variant::gateway::GatewayVariant;
use crate::star::variant::web::WebVariant;

pub mod central;
pub mod web;
pub mod gateway;

#[async_trait]
pub trait StarVariant: Send + Sync {
    fn init(&self, tx: oneshot::Sender<Result<(), Error>>) {
        tx.send(Ok(()));
    }

    fn filter(&mut self, command: &StarCommand, lane: &mut Option<&mut LaneWrapper> ) -> StarShellInstructions {
        StarShellInstructions::Handle
    }
}

pub enum StarShellInstructions {
    Ignore,
    Handle,
}

#[async_trait]
impl StarVariant for PlaceholderStarManager {

}

pub enum StarVariantCommand {
    StarSkel(StarSkel),
    Init,
    CoreRequest(CoreRequest),
    StarMessage(StarMessage),
}

impl fmt::Display for StarVariantCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = match self {
            StarVariantCommand::StarMessage(message) => {
                format!("StarMessage({})", message.payload).to_string()
            }
            StarVariantCommand::Init => "Init".to_string(),
            StarVariantCommand::StarSkel(_) => "StarSkel".to_string(),
            StarVariantCommand::CoreRequest(_) => "CoreRequest".to_string(),
        };
        write!(f, "{}", r)
    }
}

#[async_trait]
pub trait StarVariantFactory: Sync + Send {
    async fn create(&self, skel: StarSkel) -> Box<dyn StarVariant>;
}

pub struct StarVariantFactoryDefault {}

#[async_trait]
impl StarVariantFactory for StarVariantFactoryDefault {
    async fn create(&self, skel: StarSkel) -> Box<dyn StarVariant> {
        let kind = skel.info.kind.clone();
        match kind {
            StarKind::Central => Box::new(CentralVariant::new(skel.clone()).await),
            StarKind::Gateway => Box::new(GatewayVariant::new(skel.clone()).await),
            StarKind::Web => Box::new(WebVariant::new(skel.clone()).await),
            _ => Box::new(PlaceholderStarManager::new(skel.clone())),
        }
    }
}

pub struct PlaceholderStarManager {
    pub data: StarSkel,
}

impl PlaceholderStarManager {
    pub fn new(info: StarSkel) -> Self {
        PlaceholderStarManager { data: info }
    }
}
