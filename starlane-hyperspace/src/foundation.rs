use async_trait::async_trait;
use crate::registry::err::RegErr;
use crate::registry::postgres::embed::Postgres;
use crate::registry::postgres::PostgresConnectInfo;
use crate::database::{Database, LiveDatabase};
use crate::platform::PlatformConfig;

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
        config: & dyn PlatformConfig,
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

    async fn install(&self, config: & dyn PlatformConfig) -> Result<(), Self::Err> {
        Postgres::install(config).await?;
        Ok(())
    }

    async fn provision_registry(
        &self,
        config: & dyn PlatformConfig
    ) -> Result<LiveDatabase, Self::Err> {
        let db = Postgres::new(config).await?;
        let url = db.url();
        let handle = db.start().await?;
        let mut database :Database<PostgresConnectInfo>= config.registry().clone().into();
        database.settings.url= url;
        Ok(LiveDatabase { database, handle })
    }
}
