use crate::hyperspace::database::{Database, LiveDatabase};
use crate::hyperspace::platform::PlatformConfig;
use crate::hyperspace::registry::err::RegErr;
use crate::hyperspace::registry::postgres::embed::{ PostgresClusterConfig};
use crate::hyperspace::registry::postgres::{PostgresConnectInfo, PostgresRegistry};
use async_trait::async_trait;
use port_check::is_local_ipv4_port_free;
use postgresql_embedded::{PostgreSQL, Settings};
use tls_api_rustls::RustlsSessionRef;
use tokio::fs;
use crate::hyperspace::shutdown::{add_shutdown_hook, panic_shutdown};

#[async_trait]
pub trait Foundation: Send + Sync + Sized
where
    Self::Err: std::error::Error + Send + Sync,
    Self: Sized,
    Self: 'static,
    Self::RegistryFoundation: RegistryFoundation<Err = Self::Err>,
{
    type Err;

    type RegistryFoundation;

    /// install any 3rd party dependencies this foundation requires to be minimally operable
    async fn install_dependencies(&self) -> Result<(), Self::Err>;

    /// return the RegistryFoundation
    fn registry(&self) -> &Self::RegistryFoundation;
}

#[async_trait]
pub trait RegistryFoundation: DependencyFoundation<Config=Self::RegistryConfig,Err=Self::Err> where
{
    type Err;

    type RegistryConfig;

    fn dependencies( &self ) -> &Vec<impl Dependency>;
}



impl DependencyFoundation for PostgresRegistryFoundation {
    type Err = RegErr;
    type Config = PostgresRegistryFoundation::RegistryConfig;

    fn name() -> String {
        "PostgresRegistry".to_string()
    }

    fn dependency(&self) -> &impl Dependency<Err=Self::Err> {

    }

    async fn install(&self, config: &Self::Config) -> Result<(), Self::Err> {
        todo!()
    }

    async fn initialize(&self) -> Result<(), Self::Err> {
        todo!()
    }

    async fn start(&self) -> Result<LiveDatabase, Self::Err> {
        todo!()
    }
}



pub trait DependencyFoundation: Send + Sync + Sized
where
    Self::Err: std::error::Error + Send + Sync,
    Self: Sized,
    Self: 'static,
    Self::Config: Sized + Send + Sync + 'static,
{
    type Err;
    type Config;

    fn name()  -> String;

    fn dependency(&self) -> & impl Dependency<Err=Self::Err>;

    /// install the dependency in the foundation.  This may be a third party
    /// software that Starlane relies upon (such as postgres for the registry)
    async fn install(& mut self, config: &Self::Config) -> Result<(), Self::Err>;

    /// expects that `Self::install()` has installed 3rd party
    /// dependencies successfully.  `Self::initialize()` performs
    /// any initial setup that needs to occur before the dependencies can be used
    async fn initialize(& mut self) -> Result<(), Self::Err>;

    /// Start the Dependency.
    async fn start(& mut self) -> Result<LiveDatabase, Self::Err>;
}







pub struct StandAloneFoundation {
    registry: PostgresRegistryFoundation
}

impl StandAloneFoundation {
    pub fn new(registry: PostgresRegistryFoundation) -> Self {
        Self {registry}
    }
}

#[async_trait]
impl Foundation for StandAloneFoundation {
    type Err = RegErr;
    type RegistryFoundation = PostgresRegistryFoundation;

    async fn install_dependencies(&self) -> Result<(), Self::Err> {
        todo!()
    }

    fn registry(&self) -> &Self::RegistryFoundation {
        & self.registry
    }
}


pub struct PostgresDependency {
    config: PostgresClusterConfig,
    postgres: PostgreSQL,
}

impl PostgresDependency {
    pub fn new(config: PostgresClusterConfig) -> Result<Self, RegErr> {
        let mut settings = Settings::default();
        settings.data_dir = format!("{}", config.database_dir.display())
            .to_string()
            .into();
        settings.password_file = format!("{}/.password", config.database_dir.display())
            .to_string()
            .into();
        settings.port = config.port.clone();
        settings.temporary = !config.persistent;
        settings.username = config.username.clone();
        settings.password = config.password.clone();

        let postgres = PostgreSQL::new(settings);
        Ok(Self { postgres, config })
    }

    pub fn url(&self) -> String {
        self.postgres.settings().url("postgres")
    }

    /// install the postgres cluster software and setup user and password

    /*
    pub async fn
    if !is_local_ipv4_port_free(self.postgres.settings().port.clone()) {
                    let err = format!(
                        "postgres registry port '{}' is being used by another process",
                        settings.port
                    );
                    panic_shutdown(err.clone());
                    Err(RegErr::Msg(err.to_string()))?;
                }
        let _postgres = self.postgres.clone();

        add_shutdown_hook(Box::pin(async move {
            _postgres.stop().await.unwrap_or_default();
        }));

        self.postgres.start().await?;

            if !self.postgres.database_exists(&self.config.database).await? {
                self.postgres.create_database(&self.config.database).await?;
            }


        Ok(())
    }
     */

    /// as long as the Sender is alive
    pub async fn start(mut self) -> Result<LiveDatabase, RegErr> {

        if !is_local_ipv4_port_free(self.postgres.settings().port) {
            panic_shutdown(format!(
                "embedded postgres registry port '{}' is already in use",
                self.postgres.settings().port
            ));
        }

        let _postgres = self.postgres.clone();
        add_shutdown_hook(Box::pin(async move {
            println!("shutdown postgres...");
            _postgres.stop().await.unwrap();
            println!("postgres halted");
        }));

        self.postgres.start().await?;

        let live = LiveDatabase::new( )

        Ok(tx)
    }
}

impl Drop for PostgresDependency {
    fn drop(&mut self) {
        let handler = tokio::runtime::Handle::current();
        let mut postgres = self.postgres.clone();
        handler.spawn(async move {
            postgres.stop().await.unwrap_or_default();
        });
    }
}



impl DependencyFoundation for PostgresDependency {
    type Err = RegErr;
    type Config = PostgresClusterConfig;

    async fn install(&self, config: &Self::Config) -> Result<(), Self::Err> {
            /// create the data dir for this database
            fs::create_dir_all(&config.database_dir).await?;
            /// here is where postgres software is downloaded and installed (if it hasn't been already)
            self.postgres.setup().await?;
            Ok(())
    }

    async fn initialize(&self) -> Result<(), Self::Err> {
        Ok(())
    }

<<<<<<< Updated upstream
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
=======
    async fn start(&self) -> Result<LiveDatabase, Self::Err> {

        if !is_local_ipv4_port_free(self.postgres.settings().port) {
            panic_shutdown(format!(
                "embedded postgres registry port '{}' is already in use",
                self.postgres.settings().port
            ));
        }

        let _postgres = self.postgres.clone();
        add_shutdown_hook(Box::pin(async move {
            println!("shutdown postgres...");
            _postgres.stop().await.unwrap();
            println!("postgres halted");
        }));

        self.postgres.start().await?;

        let live = LiveDatabase::new( )

        Ok(tx)

>>>>>>> Stashed changes
    }
}


impl RegistryFoundation for PostgresRegistryFoundation {
    type Err = RegErr;
    type RegistryConfig = Database<PostgresClusterConfig>;

}

impl DependencyFoundation for PostgresRegistryFoundation {
    type Err = RegErr;
    type Config = PostgresRegistryFoundation::RegistryConfig;

    fn name() -> String {
        todo!()
    }

    fn dependency(&self) -> & impl Dependency<Err=Self::Err> {

    }

    async fn install(&mut self, config: &Self::Config) -> Result<(), Self::Err> {
        todo!()
    }

    async fn initialize(&mut self) -> Result<(), Self::Err> {
        todo!()
    }

    async fn start(&mut self) -> Result<LiveDatabase, Self::Err> {
        todo!()
    }
}

pub trait Dependency {
    type Err;

    fn name(&self) -> String;
    async fn install(&mut self) -> Result<(), Self::Err>;

    async fn initialize(&mut self) -> Result<(), Self::Err>;

    async fn start(&mut self) -> Result<LiveDatabase, Self::Err>;
}


pub struct PostgresRegistryFoundation {
    pub config: PostgresClusterConfig,
}

impl RegistryFoundation for PostgresRegistryFoundation {
    type Err = RegErr;
    type RegistryConfig = Database<PostgresClusterConfig>;
}
