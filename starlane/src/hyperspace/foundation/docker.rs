use crate::hyperspace::database::{Database, LiveDatabase};
use crate::hyperspace::foundation::{Dependency, DependencyKind, Foundation, FoundationErr, FoundationKind, RegistryProvider};
use crate::hyperspace::registry::postgres::embed::PostgresClusterConfig;
use crate::hyperspace::registry::postgres::PostgresConnectInfo;
use crate::hyperspace::shutdown::{add_shutdown_hook, panic_shutdown};
use crate::space::parse::VarCase;
use bollard::Docker;
use derive_builder::Builder;
use port_check::is_local_ipv4_port_free;
use postgresql_embedded::{PostgreSQL, Settings};
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::collections::HashMap;
use crate::hyperspace::foundation::config::ProtoDependencyConfig;

#[derive(Builder, Clone, Serialize, Deserialize)]
pub struct DockerDesktopFoundationConfig{
    pub postgres: PostgresClusterConfig,
    pub dependencies: HashMap<DependencyKind, Value>
}


#[derive(Clone)]
pub struct DockerDesktopFoundation {
    config: DockerDesktopFoundationConfig,
    docker: Docker,
    dependencies: HashMap<DependencyKind, dyn Dependency>
}


#[derive(Builder, Clone, Serialize, Deserialize)]
pub struct DockerConfig<C> where C: Clone+Serialize+Deserialize{
  image: String,
  config: C
}


impl DockerDesktopFoundation {
    pub fn new(docker: Docker, config: DockerDesktopFoundationConfig) -> Self {
        Self {
            docker,
            config
        }
    }
}

#[async_trait]
impl Foundation for DockerDesktopFoundation {
    fn kind(&self) -> FoundationKind {
        FoundationKind::DockerDesktop
    }

    fn dependency(&self, kind: &DependencyKind) -> Result<impl Dependency, FoundationErr> {
        todo!()
    }

    async fn install_foundation_required_dependencies(&mut self) -> Result<(), FoundationErr> {
        todo!()
    }

    async fn add_dependency(&mut self, config: ProtoDependencyConfig) -> Result<impl Dependency, FoundationErr> {
        todo!()
    }

    fn registry(&self) -> &mut impl RegistryProvider {
        todo!()
    }
}

/*
impl DependencyFoundation for DockerPostgresDependency {
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

 */



pub struct DockerPostgresDependency {
    config: PostgresClusterConfig,
    postgres: PostgreSQL,
}

impl DockerPostgresDependency {
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

impl Dependency for DockerPostgresDependency {
    fn kind() -> DependencyKind {
       DependencyKind::Postgres
    }

    fn create(args: HashMap<VarCase, String>) -> Result<impl Dependency,FoundationErr> {
        let config = Self::into_config(args)?;
        Self::new(config)
    }

    async fn install(&mut self) -> Result<(), FoundationErr> {
        self.postgres.setup().await.map_err(|err| FoundationErr::dep_err(Self::kind(), err.to_string()))
    }

    async fn provision(&mut self, kind: &ProviderKind, args: &HashMap<VarCase,String> ) -> Result<impl Provider,FoundationErr> {
       match kind {
           ProviderKind::Any => {},
           ProviderKind::Database => {},
           ProviderKind::Ext(ext) if ext.as_str() != "Database" => {
               let key = ProviderKey::new(Self::kind(), kind.clone());
               Err(FoundationErr::prov_err(key,format!("ProviderKind '{}' not available",ext).to_string()))?
           }
           _ => {}
       };

        let config = Self::into_config(args)?;

        Ok(PostgresDatabaseProvider::new(config))
    }
}


struct PostgresDatabaseProvider {
    config: Database<PostgresConnectInfo>
}

impl PostgresDatabaseProvider {

    pub fn new( config: Database<PostgresConnectInfo> ) -> PostgresDatabaseProvider  {
        Self {
            config
        }
    }
}

