use crate::hyperspace::database::{Database, LiveDatabase};
use crate::hyperspace::platform::PlatformConfig;
use crate::hyperspace::registry::err::RegErr;
use crate::hyperspace::registry::postgres::embed::Postgres;
use crate::hyperspace::registry::postgres::PostgresConnectInfo;
use async_trait::async_trait;

#[async_trait]
pub trait Foundation: Send + Sync + Sized
where
    Self::Err: std::error::Error + Send + Sync,
    Self: Sized,
    Self: 'static,
{
    type Err;

    async fn install(&self, config: &dyn PlatformConfig) -> Result<(), Self::Err>;
    async fn provision_registry(
        &self,
        config: &dyn PlatformConfig,
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

    async fn install(&self, config: &dyn PlatformConfig) -> Result<(), Self::Err> {
        Postgres::install(config).await?;
        Ok(())
    }

    async fn provision_registry(
        &self,
        config: &dyn PlatformConfig,
    ) -> Result<LiveDatabase, Self::Err> {
        let db = Postgres::new(config).await?;
        let url = db.url();
        let handle = db.start().await?;
        let mut database: Database<PostgresConnectInfo> = config.registry().clone().into();
        database.settings.url = url;
        Ok(LiveDatabase { database, handle })
    }
}
