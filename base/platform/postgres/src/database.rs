use std::str::FromStr;
use std::sync::Arc;
use async_trait::async_trait;
use sqlx::{ConnectOptions, PgPool};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use starlane_base_common::status::{Handle, Status, StatusDetail, StatusWatcher, StatusEntity};
use starlane_base_common::provider::{Provider,ProviderKindDef,ProviderKind};
use starlane_base_common::provider::err::ProviderErr;
use starlane_space::parse::Res;
use crate::service::{Connection, DbKey, Pool, PostgresConnectionConfig, PostgresService, PostgresServiceHandle};

#[derive(Clone, Eq, PartialEq)]
struct Config {
    database: String,
    connection: PostgresConnectionConfig
}

impl Config {
    pub(crate) fn connect_options(&self) -> PgConnectOptions {
       let mut options = self.connection.connect_options();
        options.database(&self.database.as_str())
    }
}

pub type PostgresDatabaseHandle = Handle<PostgresDatabase>;
pub struct PostgresDatabaseProvider {
    config: Arc<Config>,
    status: tokio::sync::watch::Sender<Status>,
}

impl PostgresDatabaseProvider {
    pub fn new(config: Arc<Config>) -> Self {
        let (status_reporter, _ ) = tokio::sync::watch::channel(Default::default());

        Self {
            config,
            status: status_reporter,
        }
    }
}

#[async_trait]
impl Provider for PostgresDatabaseProvider{
    type Config = Config;
    type Item = PostgresDatabase;

    fn kind(&self) -> ProviderKindDef {
        ProviderKindDef::PostgresService
    }

    fn config(&self) -> Arc<Self::Config> {
        self.config.clone()
    }

    async fn probe(&self) -> Result<(), ProviderErr> {
        todo!()
    }

    async fn ready(&self) -> Result<Self::Item, ProviderErr> {
        todo!()
    }
}


#[async_trait]
impl StatusEntity for PostgresDatabaseProvider{
    fn status(&self) -> Status {
        todo!()
    }

    fn status_detail(&self) -> StatusDetail {
        todo!()
    }

    fn status_watcher(&self) -> StatusWatcher {
        todo!()
    }

    async fn probe(&self) -> StatusWatcher {
        todo!()
    }
}


pub struct PostgresDatabase {
    config: Config,
    service: PostgresServiceHandle,
    pool: Pool
}

impl PostgresDatabase {

    /// create a new Postgres Connection `Pool`
    async fn new(config: Config, service: PostgresServiceHandle   ) -> Result<Self,sqlx::Error>{
            let pool= PgPool::connect_with(config.connect_options()).await?;

        Ok(Self {
            config,
            service,
            pool
        })
    }
    async fn acquire(&self ) -> Result<Connection, sqlx::Error> {
        self.service.acquire().await
    }
}

#[async_trait]
impl StatusEntity for PostgresDatabase {
    fn status(&self) -> Status {
        todo!()
    }

    fn status_detail(&self) -> StatusDetail {
        todo!()
    }

    fn status_watcher(&self) -> StatusWatcher {
        todo!()
    }

    async fn probe(&self) -> StatusWatcher {
        todo!()
    }
}

impl PostgresDatabase {
    pub fn key(&self) -> &DbKey {
        &self.service.key()
    }
}
