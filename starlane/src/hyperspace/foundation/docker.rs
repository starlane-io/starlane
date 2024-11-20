use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use once_cell::sync::Lazy;
use tokio::fs;
use port_check::is_local_ipv4_port_free;
use postgresql_embedded::{PostgreSQL, Settings};
use crate::env::StarlaneWriteLogs;
use crate::hyperspace::database::LiveDatabase;
use crate::hyperspace::foundation::{Dependency, DependencyFoundation, DependencyKey, Foundation, FoundationErr, PostgresRegistryFoundation, Provider, ProviderKey};
use crate::hyperspace::registry::err::FoundationErr;
use crate::hyperspace::registry::postgres::embed::PostgresClusterConfig;
use crate::hyperspace::shutdown::{add_shutdown_hook, panic_shutdown};
use crate::space::parse::VarCase;



type GetDep =  dyn FnMut(HashMap<VarCase,String>) -> dyn Future<Output=Result<impl Dependency,FoundationErr>> + Sync + Send+ 'static;

static DOCKER_DEPS: Lazy<HashMap<DependencyKey, GetDep>> =
    Lazy::new(|| {
        let mut deps = HashMap::new();
        deps.insert( DependencyKey::Postgres, PostgresDependency::create );
        deps
    });


#[derive(Clone)]
pub struct DockerDesktopFoundation {
    registry: PostgresRegistryFoundation,
    dependencies: HashMap<String,Box<dyn Dependency>>
}

impl DockerDesktopFoundation {
    pub fn new(registry: PostgresRegistryFoundation) -> Self {
        Self {registry}
    }
}

#[async_trait]
impl Foundation for DockerDesktopFoundation {
    type RegistryFoundation = PostgresRegistryFoundation;

    async fn install_foundation_required_dependencies(&self) -> Result<(), FoundationErr> {
        todo!()
    }

    async fn install_dependency(&self, key: &DependencyKey, args: Vec<String>) -> Result<impl Dependency, FoundationErr> {
        todo!()
    }

    fn registry(&self) -> &Self::RegistryFoundation {
        & self.registry
    }
}

impl DependencyFoundation for PostgresDependency {
    type Config = PostgresClusterConfig;

    fn name() -> String {
        todo!()
    }

    fn dependency(&self) -> &impl Dependency {
        todo!()
    }

    async fn install(&self, config: &Self::Config) -> Result<(), FoundationErr> {
            /// create the data dir for this database
            fs::create_dir_all(&config.database_dir).await?;
            /// here is where postgres software is downloaded and installed (if it hasn't been already)
            self.postgres.setup().await?;
            Ok(())
    }

    async fn initialize(&self) -> Result<(), FoundationErr> {
        Ok(())
    }


    async fn start(&self) -> Result<LiveDatabase, FoundationErr> {

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

        todo!();
    }
}



pub struct PostgresDependency {
    config: PostgresClusterConfig,
    postgres: PostgreSQL,
}

impl PostgresDependency {
    pub fn new(config: PostgresClusterConfig) -> Result<Self, FoundationErr> {
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
                    Err(FoundationErr::Msg(err.to_string()))?;
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
    pub async fn start(mut self) -> Result<LiveDatabase, FoundationErr> {

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

        let live = LiveDatabase::new( );

        Ok(tx)
    }
}

impl Dependency for PostgresDependency {
    fn key() -> DependencyKey {
       DependencyKey::Postgres
    }

    fn create(args: HashMap<String, String>) -> Result<impl Dependency,FoundationErr> {
        let map = args.to_js;
        let config: PostgresClusterConfig = serde_json::from(&args).unwrap(); //.map_err(|err| FoundationErr::dep_conf_err(Self::key(), err,args))?;
        Ok(Self::new(config))
    }

    async fn install(&mut self) -> Result<(), FoundationErr> {
        todo!()
    }

    async fn start(&mut self) -> Result<LiveDatabase, FoundationErr> {
        todo!()
    }

    async fn provider(&mut self, key: &ProviderKey, args: &HashMap<VarCase,String> ) -> Result<impl Provider,FoundationErr> {
        todo!()
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
