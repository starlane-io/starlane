use crate::starlane_hyperspace::hyperspace::registry::err::RegErr;
use crate::starlane_hyperspace::hyperspace::registry::postgres::embed::{PgEmbedSettings, Postgres};
use crate::starlane_hyperspace::hyperspace::registry::postgres::PostgresConnectInfo;
use crate::{Database, LiveDatabase, StarlaneConfig};
use std::sync::Arc;
use tokio::fs;
use tracing::instrument::WithSubscriber;

#[async_trait]
pub trait Foundation: Send + Sync + Sized
where
    Self::Err: std::error::Error + Send + Sync,
    Self: Sized,
    Self: 'static,
{
    type Err;

    async fn install(&self, config: &StarlaneConfig) -> Result<(), Self::Err>;
    async fn provision_registry(
        &self,
        config: &StarlaneConfig,
    ) -> Result<LiveDatabase, Self::Err>;
}

#[derive(Clone)]
pub struct StandAloneFoundation();

impl StandAloneFoundation {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Foundation for StandAloneFoundation {
    type Err = RegErr;

    async fn install(&self, config: &StarlaneConfig) -> Result<(), Self::Err> {
        Postgres::install(config).await?;
        Ok(())
    }

    async fn provision_registry(
        &self,
        config: &StarlaneConfig,
    ) -> Result<LiveDatabase, Self::Err> {
        let db = Postgres::new(config).await?;
        let url = db.url();
        let handle = db.start().await?;
        let mut database :Database<PostgresConnectInfo>= config.clone().registry.into();
        database.settings.url= url;
        Ok(LiveDatabase { database, handle })
    }
}
