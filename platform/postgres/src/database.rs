use async_trait::async_trait;
use base::status::{Entity, Handle, StatusProbe};
use starlane_base as base;
use std::future::Future;
use std::ops::Deref;
use sqlx::postgres::PgConnectOptions;
use crate::database::partial::pool::PostgresDatabaseConnectionPoolProvider;
/// these reexports must come from [crate::service] since they are mocks when `#[cfg(test)]`
use crate::service;
use service::{PgConnection, Pool, PoolConnection};
use crate::service::config;

pub type PostgresDatabaseHandle = Handle<dyn PostgresDatabase<DerefTarget = Pool>>;

/// tried to make [Handle] expose [Pool] by implementing
/// ```
/// # use std::ops::Deref;
/// # trait Entity {}
/// # struct Handle<E> where E: Entity+Send+Sync { entity: E };
///
/// impl <E,D> Deref for Handle<E> where E: Deref<Target=D>+Entity+Send+Sync {
///     type Target = D;
///
///     fn deref(&self) -> &Self::Target {
///         self.entity.deref()
///     }
/// }
/// ```
#[async_trait]
pub trait PostgresDatabase: Entity + PostgresDatabaseConnectionPoolProvider + StatusProbe + Send + Sync { }


/// final [starlane::config::ProviderConfig] trait definitions for [concrete::PostgresProviderConfig]
#[async_trait]
pub trait ProviderConfig: service::ProviderConfig {
}

mod concrete {
    use super::base;
    use std::future::Future;
    mod my {
        pub use super::super::*;
    }

    use crate::database::concrete::my::PostgresDatabaseConnectionPoolProvider;
    use crate::service::config::PostgresUtilizationConfig;
    use crate::service::{PgConnection, PostgresServiceHandle};
    use async_trait::async_trait;
    use starlane_hyperspace::base::provider::{Provider, ProviderKindDef};
    use sqlx::postgres::PgConnectOptions;
    use sqlx::{Connection, PgPool};
    use starlane_base::foundation::config::ProviderConfig;
    use starlane_hyperspace::base::provider;
    use starlane_base::status::{Entity, EntityReadier, EntityResult, StatusResult};
    use starlane_base::status::{Status, StatusProbe};
    use std::ops::Deref;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    #[derive(Clone, Eq, PartialEq)]
    pub struct Config {
        database: String,
        connection: PostgresUtilizationConfig,
    }

    impl Config {
        pub(crate) fn connect_options(&self) -> PgConnectOptions {
            let mut options = self.connection.connect_options();
            options.database(&self.database.as_str())
        }

        #[cfg(test)]
        pub fn mock() -> Self {
            let database = "registry".to_string();
            let connection = PostgresUtilizationConfig::mock();

            Self {
                database,
                connection,
            }
        }
    }

    impl starlane_hyperspace::base::config::ProviderConfig for Config {
        fn kind(&self) -> &ProviderKindDef {
            todo!()
        }
    }

    impl ProviderConfig for Config {}

    pub struct PostgresDatabaseProvider {
        config: Arc<Config>,
        status: tokio::sync::watch::Sender<Status>,
    }

    impl PostgresDatabaseProvider {
        pub fn new(config: Arc<Config>) -> Self {
            let (status_reporter, _) = tokio::sync::watch::channel(Default::default());

            Self {
                config,
                status: status_reporter,
            }
        }

        #[cfg(test)]
        pub fn mock() -> Self {
            let config = Arc::new(Config::mock());
            Self::new(config)
        }
    }

    impl PostgresDatabaseConnectionPoolProvider for PostgresDatabase {
        fn pool(&self) -> &my::Pool {
            &self.pool
        }
    }

    #[async_trait]
    impl my::PostgresDatabase for PostgresDatabase {}

    #[async_trait]
    impl EntityReadier for PostgresDatabaseProvider {
        type Entity = PostgresDatabase;

        async fn ready(&self) -> EntityResult<Self::Entity> {
            todo!()
        }
    }

    #[async_trait]
    impl Provider for PostgresDatabaseProvider {
        type Config = Config;

        fn provider_kind(&self) -> ProviderKindDef {
            ProviderKindDef::PostgresService
        }

        fn config(&self) -> Arc<Self::Config> {
            self.config.clone()
        }
    }

    #[async_trait]
    impl StatusProbe for PostgresDatabaseProvider {
        async fn probe(&self) -> StatusResult {
            todo!()
        }
    }

    pub struct PostgresDatabase {
        config: Config,
        service: PostgresServiceHandle,
        pool: my::Pool,
    }

    impl PostgresDatabase {
        /// create a new Postgres Connection `Pool`
        #[cfg(not(test))]
        async fn new(config: Config, service: PostgresServiceHandle) -> Result<Self, sqlx::Error> {
            let pool = PgPool::connect_with(config.connect_options()).await?;

            Ok(Self {
                config,
                service,
                pool,
            })
        }

        #[cfg(test)]
        #[cfg(feature = "test")]
        pub fn mock(service: PostgresServiceHandle) -> Self {
            let config = Config::mock();
            let pool: MockPool<sqlx::Postgres> = Pool::default();
            Self {
                config,
                service,
                pool,
            }
        }
    }

    impl Entity for PostgresDatabase {
        type DerefTarget = my::Pool;

        fn deref_target(&self) -> &Self::DerefTarget {
            todo!()
        }
    }

    #[async_trait]
    impl StatusProbe for PostgresDatabase {
        async fn probe(&self) -> StatusResult {
            #[cfg(not(test))]
            async fn ping(pool: &my::Pool) -> Result<Status, sqlx::Error> {
                pool.acquire().await?.ping().await.map(|_| Status::Ready)
            }

            #[cfg(test)]
            #[cfg(feature = "test")]
            async fn ping(pool: &my::Pool) -> Result<Status, sqlx::Error> {
                Ok(Status::Ready)
            }

            todo!()
        }
    }
}

pub mod partial {
    mod my {
        pub use super::super::*;
    }

    /// connection pool support
    pub mod pool {
        use super::my;
        pub trait PostgresDatabaseConnectionPoolProvider {
            fn pool(&self) -> &my::Pool;
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::concrete::{PostgresDatabase, PostgresDatabaseProvider};
    use starlane_base::status::Handle;
    use starlane_space::status::EntityReadier;
    use std::ops::Deref;

    #[tokio::test]
    pub async fn test_handle_deref() {

    }
}
