use std::fmt;

use crate::error::Error;
use crate::frame::{StarMessage, StarMessagePayload};
use crate::star::variant::central::CentralVariant;
use crate::star::variant::web::WebVariant;
use crate::star::{CoreRequest, StarKind, StarSkel};
use tokio::sync::oneshot;

pub mod central;
pub mod web;

#[async_trait]
pub trait StarVariant: Send + Sync {
    async fn init(&self, tx: oneshot::Sender<Result<(), Error>>);
}

#[async_trait]
impl StarVariant for PlaceholderStarManager {
    async fn init(&self, tx: oneshot::Sender<Result<(), Error>>) {
        tx.send(Ok(()));
    }
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
