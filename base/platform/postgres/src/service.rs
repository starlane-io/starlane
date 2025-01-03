//! The [Platform]  implementation of [Provider].
//!
//! [Provider] provides access to a Postgres Cluster Instance.
//!
//! This mod implements the platform [Provider] which is a [provider::Provider] that readies a
//! [PostgresServiceHandle].  Like every platform provider this [Provider] implementation
//! cannot install 3rd party extensions, a platform [provider::Provider] CAN maintain a connection pool
//! to a postgres cluster that already exists or if the [Foundation] has a [provider::Provider] definition of
//! with a matching [ProviderKindDef]... the [Foundation] [provider::Provider] can be a dependency of the
//! [Platform]

#[cfg(not(test))]
pub use types::*;
#[cfg(test)]
pub use tests::types::*;

#[cfg(not(test))]
pub(super) mod types {
    pub type Pool = sqlx::Pool<sqlx::Postgres>;
    pub type PoolConnection = sqlx::pool::PoolConnection<sqlx::Postgres>;
    pub type PgConnection = sqlx::postgres::PgConnection;
}

/// maybe add proper postgres type constraints on the following stuff:
pub type Username = VarCase;
pub type Password = String;
pub type DbName = VarCase;
/// default to 'public'
pub type SchemaName = VarCase;
pub type Hostname = Domain;

use std::fmt::Display;
use std::sync::Arc;
use async_trait::async_trait;
use sqlx::postgres::PgConnectOptions;
use starlane_base::provider;
use starlane_space::parse::{Domain, VarCase};
use starlane_space::status::{Entity, Handle, StatusProbe};
use starlane_base::Foundation;
use starlane_base::Platform;
use starlane_base::kind::ProviderKindDef;
use starlane_base as base;

/// final [starlane::config::ProviderConfig] trait definitions for [concrete::PostgresProviderConfig]
#[async_trait]
pub trait ProviderConfig: config::ProviderConfig {
    fn utilization_config(&self) ->  & config::PostgresUtilizationConfig;

    /// reexport [config::PostgresUtilizationConfig::connect_options]
    fn connect_options(&self) -> PgConnectOptions {
        self.utilization_config().connect_options()
    }
}

/// final [provider::Provider] trait definitions for [concrete::PostgresServiceProvider]
#[async_trait]
pub trait Provider:  provider::Provider<Entity=Arc<dyn PostgresService>>  {
    type Config: ProviderConfig + ?Sized;
}


/// trait implementation [Provider::Entity]
#[async_trait]
pub trait PostgresService : Entity+StatusProbe+Send+Sync { }


pub type PostgresServiceHandle = Handle<dyn PostgresService>;


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

pub mod config {
    mod my { pub use super::super::*; }
    use std::str::FromStr;
    use sqlx::postgres::PgConnectOptions;
    use crate::err::PostErr;
    use starlane_base as base;
    use crate::service::{Hostname, Password, Username};

    pub trait ProviderConfig: base::config::ProviderConfig { }

    #[derive(Clone, Eq, PartialEq)]
    pub struct PostgresUtilizationConfig {
        pub host: my::Hostname,
        pub port: u16,
        pub username: my::Username,
        pub password: String,
    }

    impl PostgresUtilizationConfig {
        pub fn new<User, Pass>(
            host: my::Hostname,
            port: u16,
            username: User,
            password: Pass,
        ) -> Result<Self, PostErr>
        where
            User: AsRef<str>,
            Pass: ToString,
        {
            let username = my::Username::from_str(username.as_ref())?;
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


        #[cfg(test)]
        pub fn mock() -> Self {
            Self {
                host: Hostname::from_str("mock").unwrap(),
                port: 5432u16,
                username: Username::from_str("postgres").unwrap(),
                password: Password::from_str("its_a_secret").unwrap(),
            }
        }
    }


}



pub mod partial {
    pub mod mount {
    }
}




mod concrete {
    use std::fmt::Display;
    use std::ops::Deref;
    use std::sync::Arc;
    use async_trait::async_trait;
    use starlane_base::provider::{Manager, Provider, ProviderKindDef};
    use std::str::FromStr;
    use sqlx;
    use sqlx::{ConnectOptions, Connection};
    use tokio::sync::Mutex;
    use starlane_base::Foundation;
    use starlane_base::Platform;
    use starlane_space::status;
    use starlane_space::status::{Entity, EntityReadier, StatusReporter, StatusResult};
    use status::{ReadyResult, Handle, Status, StatusDetail, StatusProbe, StatusWatcher};
    use super::config;
    use super::base;

    use super::{Pool,PoolConnection,PgConnection};

    use config::PostgresUtilizationConfig;
    use starlane_base::config::ProviderConfig;

    pub mod my { pub use super::super::*; }


    pub struct PostgresServiceProvider {
        config: Arc<PostgresProviderConfig>,
        status_reporter: StatusReporter
    }

    impl PostgresServiceProvider {
        pub fn new(config: Arc<PostgresProviderConfig>) -> PostgresServiceProvider {
            let status_reporter = status::status_reporter();

            Self {
                config,
                status_reporter
            }
        }

        #[cfg(test)]
        pub fn mock() -> PostgresServiceProvider {

          let config = Arc::new(PostgresProviderConfig::mock());
           Self::new(config)
        }

    }

    #[async_trait]
    impl EntityReadier for PostgresServiceProvider {
        type Entity = dyn my::PostgresService;

        async fn ready(&self) -> ReadyResult<Self::Entity> {
            todo!()
        }
    }

    #[async_trait]
    impl Provider for PostgresServiceProvider {
        type Config = PostgresProviderConfig;

        fn kind(&self) -> ProviderKindDef {
            ProviderKindDef::PostgresService
        }

        fn config(&self) -> Arc<Self::Config> {
            self.config.clone()
        }
    }


    #[async_trait]
    impl StatusProbe for PostgresServiceProvider {
        async fn probe(&self) -> status::StatusResult {
            todo!()
        }
    }


    /// the [StatusProbe] implementation which tracks with a Postgres Connection [Pool].
    /// With any [StatusProbe] the goal is to get to a [Status::Ready] state.  [PostgresService]
    /// should abstract the specific [Manager] details.  A [PostgresService] may be a
    /// [Manager::Foundation] in which the [PostgresService] would be responsible for
    /// downloading, installing, initializing and starting Postgres before it creates the pool or if
    /// [Manager::External] then Starlane's [Platform] is only responsible for maintaining
    /// a connection pool to the given Postgres Cluster
    ///
    #[derive(Clone)]
    pub struct PostgresService {
        config: PostgresProviderConfig,
        connection: Arc<Mutex<PgConnection>>
    }

    impl Entity for PostgresService {}

    #[async_trait]
    impl my::PostgresService for PostgresService {}



    impl PostgresService {
        #[cfg(not(test))]
        async fn new(config: PostgresProviderConfig) -> Result<Self, sqlx::Error> {
            let connection = Arc::new(Mutex::new(config.connect_options().connect().await?));
            Ok(Self {
                config,
                connection
            })
        }

        #[cfg(test)]
        pub fn mock() -> Self {
            let connection = Arc::new(Mutex::new(PgConnection::default()));
            let config = PostgresProviderConfig::mock();
            Self {
                config,
                connection
            }
        }
    }

    #[async_trait]
    impl StatusProbe for PostgresService {
        async fn probe(&self) -> StatusResult {
            todo!()
        }
    }


    #[derive(Clone, Eq, PartialEq)]
    pub struct PostgresProviderConfig {
        connection_info: my::config::PostgresUtilizationConfig
    }


    #[cfg(test)]
    impl PostgresProviderConfig {
        pub fn mock() -> Self {
            Self {
                connection_info: PostgresUtilizationConfig::mock()
            }
        }
    }

    #[async_trait]
    impl my::ProviderConfig for PostgresProviderConfig {
        fn utilization_config(&self) -> &PostgresUtilizationConfig {
            &self.connection_info
        }


    }

    impl Deref for PostgresProviderConfig {
        type Target = my::config::PostgresUtilizationConfig;

        fn deref(&self) -> &Self::Target {
            &self.connection_info
        }
    }


    #[async_trait]
    impl base::config::ProviderConfig for PostgresProviderConfig {
        fn kind(&self) -> &ProviderKindDef {
            todo!()
        }
    }

    #[async_trait]
    impl config::ProviderConfig for PostgresProviderConfig { }
}




#[cfg(test)]
pub(crate) mod tests {
    use base::status::Handle;
    use crate::service::concrete::my::base;
    use super::concrete::{PostgresService, PostgresServiceProvider};

    pub(crate) mod types {
        use std::marker::PhantomData;

        pub type Pool = MockPool<sqlx::Postgres>;
        pub type PoolConnection = MockPoolConnection<sqlx::Postgres>;
        pub type PgConnection = MockPgConnection<sqlx::Postgres>;

        pub struct MockPool<Db> (PhantomData<Db>) where Db : sqlx::Database;

        impl Default for MockPool<sqlx::Postgres> {
            fn default() -> Self {
                Self(PhantomData::default())
            }
        }

        pub struct MockPoolConnection<Db> (PhantomData<Db>) where Db : sqlx::Database;
        impl Default for MockPoolConnection<sqlx::Postgres> {
            fn default() -> Self {
                Self(PhantomData::default())
            }
        }

        pub struct MockPgConnection<Db> (PhantomData<Db>) where Db : sqlx::Database;

        impl Default for MockPgConnection<sqlx::Postgres> {
            fn default() -> Self {
                Self(PhantomData::default())
            }
        }
    }

    #[tokio::test]
    pub async fn test_handle_deref() {
        let service = PostgresService::mock();
        let handle = Handle::mock(service);
    }
}

/*


#[cfg(test)]
pub mod tests {
    use std::future::Future;
    use std::ops::Deref;
    use sqlx::Database;

    use MockPoolConnection as PoolConnection;
    type Pool = crate::database::tests::MockPool<sqlx::Postgres>;
    type PoolConnection = crate::database::tests::MockPoolConnection<sqlx::Postgres>;

    use starlane_base::status::{Entity, Handle, StatusProbe, StatusResult};

    pub struct MockDatabase{
        pool: Pool
    }

    impl Entity for crate::database::tests::MockDatabase {}

    impl StatusProbe for crate::database::tests::MockDatabase {
        async fn probe(&self) -> StatusResult {
            todo!()
        }
    }

    impl Deref<Target=Pool> for crate::database::tests::MockDatabase {
        type Target = Pool;

        fn deref(&self) -> &Self::Target {
            todo!()
        }
    }

    pub struct MockPool<Db> where Db: Database;

    impl <Db> Entity for crate::database::tests::MockPool<Db> where Db: Database { }

    impl <Db> Default for crate::database::tests::MockPool<Db>
    where Db: Database {
        fn default() -> Self {
            Self
        }
    }

    impl <Db> crate::database::tests::MockPool<Db>
    where Db: Database{

        pub fn acquire(&self) -> impl Future<Output = Result<PoolConnection<Db>, sqlx::Error>> + 'static {
            async move { Ok(PoolConnection::default()) }
        }

    }

    pub struct MockPoolConnection<Db> where Db: Database;
    impl <Db> Default for crate::database::tests::MockPoolConnection<Db>
    where Db: Database {
        fn default() -> Self {
            Self
        }
    }


    #[tokio::test]
    pub async fn test_handle_deref() {
        let pool : crate::database::tests::MockPool<sqlx::Postgres> = Pool::default();
        let database = super::PostgresDatabase::new(Default:)
        let handle = Handle::mock(pool);

        let deref = handle.deref();

    }
}

 */