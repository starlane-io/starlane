use crate::hyperspace::database::{Database, LiveDatabase};
use crate::hyperspace::foundation::config::{Config, FoundationSubConfig, ProtoConfig, ProtoDependencyConfig, ProtoProviderConfig};
use crate::hyperspace::foundation::{CreateDep, CreateFoundation, Dependency, DependencyKind, Foundation, FoundationErr, FoundationKind, Provider, ProviderKind, RegistryProvider};
use crate::hyperspace::registry::postgres::embed::PostgresClusterConfig;
use crate::hyperspace::registry::postgres::PostgresConnectInfo;
use crate::hyperspace::shutdown::{add_shutdown_hook, panic_shutdown};
use bollard::Docker;
use derive_builder::Builder;
use port_check::is_local_ipv4_port_free;
use postgresql_embedded::{PostgreSQL, Settings};
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::collections::{HashMap, HashSet};
use once_cell::sync::Lazy;

static DEPENDENCIES: Lazy<HashMap<DependencyKind, CreateDep>> =
    Lazy::new(|| {
        let mut dependencies = HashMap::new();
        dependencies.insert(DependencyKind::Docker, DockerDependency::create );
        dependencies
    });


pub type ProtoDockerDesktopFoundationConfig = Config<FoundationKind,Value>;





#[derive(Builder, Clone, Serialize, Deserialize)]
pub struct DockerConfig<C>
where
    C: Clone + Serialize + Deserialize,
{
    image: String,
    config: C,
}

#[derive(Builder,Clone, Serialize, Deserialize)]
pub struct DockerDesktopFoundationSubConfig {
    pub registry: Database<PostgresClusterConfig>,
    pub dependencies: HashMap<DependencyKind, Value>,
}
pub struct DockerDesktopFoundation {
    config: Config<FoundationKind,DockerDesktopFoundationSubConfig>,
    dependencies: HashMap<DependencyKind, dyn Dependency>,
}


impl DockerDesktopFoundation {
    pub(super) fn create(config: impl ProtoConfig) -> Result<impl Foundation, FoundationErr> {
        let config = config.parse(FoundationKind::DockerDesktop)?;
        Ok(Self {
            config,
            ..Default::default()
        })
    }
}

#[async_trait]
impl Foundation for DockerDesktopFoundation {
    fn kind(&self) -> FoundationKind {
        FoundationKind::DockerDesktop
    }

    fn dependency(&self, kind: &DependencyKind) -> Result<&impl Dependency, FoundationErr> {
       self.dependencies.get(kind).ok_or_else(|| FoundationErr::dep_not_available(kind.clone()))
    }

    async fn install_foundation_required_dependencies(&mut self) -> Result<(), FoundationErr> {
        ///...
        Ok(())
    }

    async fn add_dependency(
        &mut self,
        config: ProtoDependencyConfig,
    ) -> Result<impl Dependency, FoundationErr> {
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


#[derive(Builder,Clone, Serialize, Deserialize)]
pub struct DockerDependencySubConfig {
}

struct DockerDependency {
  config: Config<DependencyKind,DockerDependencySubConfig>
}

impl DockerDependency {
    pub(super) fn create(config: impl ProtoConfig) -> Result<impl Dependency, FoundationErr> {
            let config = config.parse(DependencyKind::Docker)?;
            Ok(Self {
                config,
                ..Default::default()
            })

    }
}


impl Dependency for DockerDependency {


    fn kind(&self) -> &DependencyKind {
        & DependencyKind::Docker
    }



    async fn install(&self) -> Result<(), FoundationErr> {
        match Docker::connect_with_defaults() {
            Ok(_) => {
                // Docker was accessed normally and is therefor both installed and service is running...
                Ok(())
            }
            Err(err) => {
                Err(FoundationErr::user_action_required("Dependency", self.kind().to_string(), "make sure Docker is installed and running on this machine", format!("Starlane foundation '{}' needs Docker to facilitate the underlying infrastructure.  Please follow these instructions to install and run Docker: `https://www.docker.com/` then rerun the Starlane installation process", FoundationKind::DockerDesktop )))
            }
        }
    }

    async fn provision(&self, config: ProtoProviderConfig) -> Result<impl Provider,FoundationErr> {
        if ProviderKind::DockerDaemon == config.kind  {
            todo!()
        } else {
            Err(FoundationErr::provider_not_available( config.kind.clone() ))
        }
    }


    /// implementers of this Trait should provide a vec of valid provider kinds
    fn provider_kinds(&self) -> HashSet<&'static str> {
        HashSet::new()
    }

}

#[derive(Builder,Clone, Serialize, Deserialize)]
pub struct DockerProviderSubConfig {
}

struct DockerProvider {
    config: Config<ProviderKind,DockerProviderSubConfig>
}

impl DockerProvider {

    pub(super) fn create(config: impl ProtoConfig) -> Result<impl Provider, FoundationErr> {
        let config = config.parse(ProviderKind::DockerDaemon)?;
        Ok(Self {
            config,
            ..Default::default()
        })

    }
}

impl Provider for DockerProvider {
    async fn initialize(&mut self) -> Result<(), FoundationErr> {
        todo!()
    }
}





pub struct DockerPostgresDependency {
    config: PostgresClusterConfig,
    postgres: PostgreSQL,
}

impl DockerPostgresDependency {
    pub fn new(config: PostgresClusterConfig) -> Result<Self, FoundationErr> {
        todo!();
        /*
        let mut settings = Settings::default();
        settings.data_dir = format!("{}", config.database_dir.display())
            .to_string()
            .into();
        settings.password_file = format!("{}/.password", config.database_dir.display())
            .to_string()
            .into();
        settings.port = config.port.clone();
        settings.temporary = false;
        settings.username = config.username.clone();
        settings.password = config.password.clone();

        let postgres = PostgreSQL::new(settings);
        Ok(Self { postgres, config })

         */
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
        todo!();
        /*
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

        let live = LiveDatabase::new();

        Ok(tx)

         */
    }
}



struct PostgresDatabaseProvider {
    config: Database<PostgresConnectInfo>,
}

impl PostgresDatabaseProvider {
    pub fn new(config: Database<PostgresConnectInfo>) -> PostgresDatabaseProvider {
        Self { config }
    }
}
