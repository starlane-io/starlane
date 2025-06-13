use crate::database::partial::pool::PostgresDatabaseConnectionPoolProvider;
/// these reexports must come from [crate::service] since they are mocks when `#[cfg(test)]`
use crate::service;
use async_trait::async_trait;
use base::status::{Entity, Handle, StatusProbe};
use starlane_base as base;
use std::future::Future;
use std::ops::Deref;

pub type PostgresDatabaseHandle = Handle<dyn PostgresDatabase>;

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
    use std::future::Future;
    mod my {
        pub use super::super::*;
    }

    use crate::database::concrete::my::PostgresDatabaseConnectionPoolProvider;
    use crate::service::config::PostgresUtilizationConfig;
    use crate::service::{Pool, PostgresServiceHandle};
    use async_trait::async_trait;
    use sqlx::postgres::PgConnectOptions;
    use sqlx::{Connection, PgPool};
    use starlane_base::foundation::config::ProviderConfig;
    use starlane_base::status::{Entity, EntityReadier, EntityResult, StatusResult};
    use starlane_base::status::{Status, StatusProbe};
    use starlane_hyperspace::base::config::BaseSubConfig;
    use starlane_hyperspace::base::provider::Provider;
    use starlane_hyperspace::base::{provider, BaseSub};
    use std::ops::Deref;
    use std::sync::Arc;

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

    impl provider::config::ProviderConfig for Config { }

    impl BaseSubConfig for Config { }

    impl starlane_hyperspace::base::config::ProviderConfig for Config { }

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
        fn pool(&self) -> &Pool {
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

    impl BaseSub for PostgresDatabaseProvider { }

    #[async_trait]
    impl Provider for PostgresDatabaseProvider { }

    #[async_trait]
    impl StatusProbe for PostgresDatabaseProvider {
        async fn probe(&self) -> StatusResult {
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
                service, pool,
            }
        }
    }

    impl Entity for PostgresDatabase { }
    

    #[async_trait]
    impl StatusProbe for PostgresDatabase {
        async fn probe(&self) -> StatusResult {
            #[cfg(not(test))]
            async fn ping(pool: &Pool) -> Result<Status, sqlx::Error> {
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
        use crate::service::Pool;
        pub trait PostgresDatabaseConnectionPoolProvider {
            fn pool(&self) -> &Pool;
        }
    }
}

#[cfg(test)]
pub mod tests {
    #[tokio::test]
    pub async fn test_handle_deref() {

    }
}
