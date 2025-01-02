use starlane_hyperspace::provider::{Manager, Provider, ProviderKindDef};
use starlane_space::err::ParseErrs;
use starlane_space::parse::{Domain, VarCase};
use starlane_space::status::{Handle, Status, StatusDetail, StatusEntity, StatusWatcher};
use std::fmt::Display;
use std::str::FromStr;
use std::sync::Arc;
use async_trait::async_trait;
use starlane_hyperspace::provider::config::ProviderConfig;
use starlane_hyperspace::provider::err::ProviderErr;

pub type PostgresServiceHandle = Handle<PostgresServiceStub>;


pub struct PostgresService {
    config: Arc<Config>,
    status_reporter: tokio::sync::watch::Sender<Status>,
}

impl PostgresService {
    pub fn new(config: Arc<Config>) -> PostgresService {
        let (status_reporter, _ ) = tokio::sync::watch::channel(Default::default());


        Self {
            config,
            status_reporter,
        }
    }
}

#[async_trait]
impl Provider for PostgresService {
    type Config = Config;
    type Item = PostgresServiceHandle;

    fn kind(&self) -> ProviderKindDef {
        ProviderKindDef::Service
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

impl StatusEntity for PostgresService {
    fn status(&self) -> StatusDetail {
        todo!()
    }

    fn status_detail(&self) -> StatusDetail {
        todo!()
    }

    fn status_watcher(&self) -> StatusWatcher {
        todo!()
    }

    fn probe(&self) -> StatusWatcher {
        todo!()
    }

    fn start(&self) -> StatusWatcher {
        todo!()
    }
}

/// the [StatusEntity] implementation which tracks with a Postgres Connection Pool.
/// With any [StatusEntity] the goal is to get to a [Status::Ready] state.  [PostgresServiceStub]
/// should abstract the specific [Manager] details.  A [PostgresServiceStub] may be a
/// [Manager::Foundation] in which the [PostgresServiceStub] would be responsible for
/// downloading, installing, initializing and starting Postgres before it creates the pool or if
/// [Manager::External] then Starlane's [Platform] is only responsible for maintaining
/// a connection pool to the given Postgres Cluster
pub struct PostgresServiceStub {
    key: DbKey,
    connection_info: Config,
}

impl StatusEntity for PostgresServiceStub {
    fn status(&self) -> StatusDetail {
        todo!()
    }

    fn status_detail(&self) -> StatusDetail {
        todo!()
    }

    fn status_watcher(&self) -> StatusWatcher {
        todo!()
    }

    fn probe(&self) -> StatusWatcher {
        todo!()
    }

    fn start(&self) -> StatusWatcher {
        todo!()
    }
}

impl PostgresServiceStub {
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





#[derive(Debug, Clone, thiserror::Error)]
pub enum PostErr {
    #[error("{0}")]
    ParseErrs(#[from] ParseErrs),
}



#[derive(Clone, Eq, PartialEq)]
pub struct Config {
    pool: PostgresConnectionConfig
}

impl Config {

}

impl ProviderConfig for Config {
    fn kind(&self) -> &ProviderKindDef {
        &ProviderKindDef::Service
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


}

/*
let pool = PgPoolOptions::new()
.max_connections(5)
.connect(db.database.to_uri().as_str())
.await?;
 */

/*
#[derive(Clone)]
pub struct LiveDatabase {
    pub database: Database<PostgresConnectInfo>,
    tx: tokio::sync::mpsc::Sender<()>,
}

impl LiveDatabase {
    pub fn new(database: Database<PostgresConnectInfo>, tx: tokio::sync::mpsc::Sender<()>) -> Self {
        Self { database, tx }
    }
}

impl Database<PostgresConnectInfo> {
    pub fn from_con<D, S>(
        database: D,
        schema: S,
        info: PostgresConnectInfo,
    ) -> Database<PostgresConnectInfo>
    where
        D: ToString,
        S: ToString,
    {
        Database::new(database, schema, info)
    }

    pub fn to_key(&self) -> PostgresDbKey {
        PostgresDbKey {
            url: self.url.clone(),
            user: self.user.clone(),
            database: self.database.clone(),
        }
    }

    pub fn to_uri(&self) -> String {
        /*
        format!(
            "postgres://{}:{}@{}/{}",
            self.user, self.password, self.url, self.database
        )

         */
        self.url.clone()
    }
}

 */
