use crate::star::{CentralCommand};
use crate::error::Error;
use std::sync::Arc;
use crate::app::{AppMeta, Application, AppMessage, AppCreateController, AppDestroy};
use crate::label::Labels;
use crate::frame::ActorMessage;
use crate::actor::Actor;
use crate::app::AppContext;

pub struct SystemAppCentral
{
}

#[async_trait]
impl Application for SystemAppCentral
{
    async fn create(&self, context: &AppContext, create: AppCreateController) -> Result<Labels, Error> {
        let mut labels = Labels::new();
        labels.insert("app".to_string(), "system".to_string() );
        Ok(labels)
    }

    async fn destroy(&self, context: &AppContext, destroy: AppDestroy) -> Result<(), Error> {
        Ok(())
    }

    async fn handle_app_command(&self, context: &AppContext, command: AppMessage) -> Result<(), Error> {
        Ok(())
    }

    async fn handle_actor_message(&self, context: &AppContext, actor: &mut Actor, message: ActorMessage) -> Result<(), Error> {
        Ok(())
    }
}