use std::sync::Arc;
use tracing::instrument::WithSubscriber;
use crate::{Database, LiveDatabase};
use crate::registry::err::RegErr;
use crate::registry::postgres::embed::{PgEmbedSettings, Postgres};
use crate::registry::postgres::PostgresConnectInfo;

#[async_trait]
pub trait Foundation: Send + Sync + Sized
where
    Self::Err: std::error::Error + Send + Sync,
    Self: Sized,
    Self: 'static,
{
    type Err;

    async fn provision_registry(&self, config: Database<PgEmbedSettings> ) -> Result<LiveDatabase, Self::Err>;

}


#[derive(Clone)]
pub struct StandAloneFoundation();


impl StandAloneFoundation {
    pub fn new() -> Self {
        Self{}
    }
}

#[async_trait]
impl Foundation for StandAloneFoundation {
    type Err = RegErr;

    async fn provision_registry(&self, config: Database<PgEmbedSettings>) -> Result<LiveDatabase, Self::Err> {
        let db =Postgres::new(config.clone()).await?;
        let handle = db.start().await?;
        let database = config.into();
        Ok(LiveDatabase {
            database,
            handle
        })
    }
}