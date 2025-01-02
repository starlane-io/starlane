use std::fmt::Display;
use std::ops::Deref;
use starlane_space::parse::{Domain, VarCase};
use std::sync::Arc;
use async_trait::async_trait;
use starlane_base_common::config::ProviderConfig;
use starlane_base_common::provider::{Manager, Provider, ProviderKindDef};
use starlane_base_common::provider::err::ProviderErr;
use std::str::FromStr;
use sqlx;
use sqlx::{ConnectOptions, Connection};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use tokio::sync::Mutex;
use starlane_base_common::Foundation;
use starlane_base_common::platform::prelude::Platform;
use starlane_base_common::status::{Handle, Status, StatusDetail, StatusEntity, StatusWatcher};
use crate::err::PostErr;

pub type Pool = sqlx::Pool<sqlx::Postgres>;
pub type Con = sqlx::pool::PoolConnection<sqlx::Postgres>;




/// The [Platform]  implementation of [PostgresServiceProvider].
///
/// [PostgresServiceProvider] provides access to a Postgres Cluster Instance.
///
/// This mod implements the platform [PostgresServiceProvider] which is a [Provider] that readies a
/// [PostgresServiceHandle].  Like every platform provider this [PostgresServiceProvider] implementation
/// cannot install 3rd party extensions, a platform [Provider] CAN maintain a connection pool
/// to a postgres cluster that already exists or if the [Foundation] has a [Provider] definition of
/// with a matching [ProviderKindDef]... the [Foundation] [Provider] can be a dependency of the
/// [Platform]
pub type PostgresServiceHandle = Handle<PostgresService>;

pub struct PostgresServiceProvider {
    config: Arc<Config>,
    status: tokio::sync::watch::Sender<Status>,
}

impl PostgresServiceProvider {
    pub fn new(config: Arc<Config>) -> PostgresServiceProvider {
        let (status_reporter, _ ) = tokio::sync::watch::channel(Default::default());

        Self {
            config,
            status: status_reporter,
        }
    }
}

#[async_trait]
impl Provider for PostgresServiceProvider {
    type Config = Config;
    type Item = PostgresServiceHandle;

    fn kind(&self) -> ProviderKindDef {
        ProviderKindDef::PostgresService
    }

    fn config(&self) -> Arc<Self::Config> {
        self.config.clone()
    }

    async fn probe(&self) -> Status {
        todo!()
    }

    async fn ready(&self) -> Result<Self::Item, ProviderErr> {
        todo!()
    }
}


#[async_trait]
impl StatusEntity for PostgresServiceProvider {
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

/// the [StatusEntity] implementation which tracks with a Postgres Connection Pool.
/// With any [StatusEntity] the goal is to get to a [Status::Ready] state.  [PostgresService]
/// should abstract the specific [Manager] details.  A [PostgresService] may be a
/// [Manager::Foundation] in which the [PostgresService] would be responsible for
/// downloading, installing, initializing and starting Postgres before it creates the pool or if
/// [Manager::External] then Starlane's [Platform] is only responsible for maintaining
/// a connection pool to the given Postgres Cluster
pub struct PostgresService {
    config: Config,
    connection: Mutex<sqlx::PgConnection>
}



impl PostgresService {

    async fn new( config: Config ) -> Result<Self,sqlx::Error> {
        let connection = Mutex::new(config.connect_options().connect().await?);
        Ok(Self {
            config,
            connection
        })
    }



}

#[async_trait]
impl StatusEntity for PostgresService {
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
        /// need to normalize the [PostgresService::probe]
        self.connection.lock().await.ping().await.unwrap();
        todo!()
    }
}

impl PostgresService {
    pub fn key(&self) -> &DbKey {
        &self.key
    }
}

/// maybe add proper postgres type constraints on the following stuff:
pub type Username = VarCase;
pub type Password = String;
pub type DbName = VarCase;
/// default to 'public'
pub type SchemaName = VarCase;
pub type Hostname = Domain;

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct DbKey {
    pub host: Hostname,
    pub user: Username,
    pub database: DbName,
    /// default to public if [None]
    pub schema: Option<SchemaName>,
}

impl Display for DbKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            format!("{}:{}@{}", self.user, self.database, self.host)
        )
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct Config {
    connection_info: PostgresConnectionConfig
}

impl Deref for Config {
    type Target = PostgresConnectionConfig;

    fn deref(&self) -> &Self::Target {
        &self.connection_info
    }
}

impl Config {

}

impl ProviderConfig for Config {
    fn kind(&self) -> &ProviderKindDef {
        &ProviderKindDef::PostgresService
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct PostgresConnectionConfig {
    pub host: Hostname,
    pub port: u16,
    pub username: Username,
    pub password: String,
}

impl PostgresConnectionConfig{
    pub fn new<User, Pass>(
        host: Hostname,
        port: u16,
        username: User,
        password: Pass,
    ) -> Result<Self, PostErr>
    where
        User: AsRef<str>,
        Pass: ToString,
    {
        let username = Username::from_str(username.as_ref())?;
        let password = password.to_string();
        Ok(Self {
            host,
            username,
            password,
            port,
        })
    }

    pub(crate) fn connect_options(&self) -> PgConnectOptions {
        PgConnectOptions::new()
            .host(self.host.as_str())
            .port(self.port.clone())
            .username(self.username.as_str())
            .password(self.password.as_str())
    }



    /*
    pub fn to_uri(&self) -> String {
        format!(
            "postgres://{}:{}@{}/{}",
            self.username, self.password, self.host, self.database
        )
    }


     */

    pub fn to_uri(&self) -> String {
        format!(
            "postgres://{}:{}@{}",
            self.username, self.password, self.host
        )
    }
}