#[cfg(feature = "postgres")]
use crate::env::{
    STARLANE_REGISTRY_DATABASE, STARLANE_REGISTRY_PASSWORD, STARLANE_REGISTRY_URL,
    STARLANE_REGISTRY_USER,
};
#[cfg(feature = "postgres")]
use crate::registry::postgres::{
    PostgresConnectInfo, PostgresPlatform, PostgresRegistry, PostgresRegistryContext,
    PostgresRegistryContextHandle,
};

use crate::driver::base::BaseDriverFactory;
use crate::driver::control::ControlDriverFactory;
use crate::driver::root::RootDriverFactory;
use crate::driver::space::SpaceDriverFactory;
use crate::driver::{DriverAvail, DriversBuilder};
use starlane::space::artifact::asynch::Artifacts;
use starlane::space::kind::StarSub;
use starlane::space::loc::{MachineName, StarKey};
use starlane::space::log::{root_logger, RootLogger};
use starlane::space::point::Point;
use std::fs;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

use crate::driver::star::Star;
use crate::env::{STARLANE_CONTROL_PORT, STARLANE_DATA_DIR, STARLANE_REGISTRY_SCHEMA};
use crate::err::HypErr;
use crate::hyperlane::tcp::{CertGenerator, HyperlaneTcpServer};
use crate::hyperlane::{AnonHyperAuthenticator, HyperGateSelector, LocalHyperwayGateJumper};
use crate::hyperspace::machine::MachineTemplate;
use crate::hyperspace::reg::{Registry, RegistryWrapper};
use crate::platform::Platform;
use crate::registry::err::RegErr;
use crate::registry::mem::registry::{MemoryRegistry, MemoryRegistryCtx};
use crate::registry::postgres::embed::{PgEmbedSettings, Postgres};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::ops::Deref;
use crate::registry::postgres::PostgresDbKey;

#[derive(Clone, Serialize, Deserialize)]
pub struct StarlaneConfig {
    pub registry: PgRegistryConfig,
}

impl Default for StarlaneConfig {
    fn default() -> StarlaneConfig {
        Self {
            registry: PgRegistryConfig::default(),
        }
    }
}

#[derive(Clone)]
pub struct Starlane {
    config: StarlaneConfig,
    artifacts: Artifacts,
    registry: Registry,
}

pub enum RegistryConfig {
    #[cfg(feature = "postgres")]
    Postgres(PgRegistryConfig),
}
#[cfg(feature = "postgres")]
#[derive(Clone, Serialize, Deserialize)]
pub enum PgRegistryConfig {
    #[cfg(feature = "postgres-embedded")]
    Embedded(Database<PgEmbedSettings>),
    External(Database<PostgresConnectInfo>),
}

#[cfg(feature = "postgres")]
impl Default for PgRegistryConfig {
    fn default() -> Self {
        let database = Database::new(
            STARLANE_REGISTRY_DATABASE.to_string(),
            STARLANE_REGISTRY_SCHEMA.to_string(),
            PgEmbedSettings::default(),
        );
        Self::Embedded(database)
    }
}

#[derive(Clone, Serialize, Deserialize,Eq,PartialEq,Hash)]
pub struct Database<S> {
    pub database: String,
    pub schema: String,
    pub settings: S,
    pub nuke: bool
}

impl<Info> Database<Info> {
    pub fn new<D, S>(database: D, schema: S, settings: Info) -> Database<Info>
    where
        D: ToString,
        S: ToString,
    {
        let database = database.to_string();
        let schema = schema.to_string();
        Database {
            database,
            settings,
            schema,
            nuke: false
        }
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
        format!(
            "postgres://{}:{}@{}/{}",
            self.user, self.password, self.url, self.database
        )
    }

}

impl Database<PgEmbedSettings> {
    pub fn from_embed<D, S>(
        database: D,
        schema: S,
        settings: PgEmbedSettings,
    ) -> Database<PgEmbedSettings>
    where
        D: ToString,
        S: ToString,
    {
        Self::new(database, schema, settings)
    }


    pub fn to_key(&self) -> PostgresDbKey {
        PostgresDbKey {
            url: "localhost".to_string(),
            user: self.settings.user.clone(),
            database: self.database.clone(),
        }
    }

    pub fn to_uri(&self) -> String {
        format!(
            "postgres://{}:{}@localhost/{}",
            self.user, self.password, self.database
        )
    }

}

impl<S> Deref for Database<S> {
    type Target = S;

    fn deref(&self) -> &Self::Target {
        &self.settings
    }
}

#[cfg(feature = "postgres")]
impl Starlane {
    pub async fn new(config: PgRegistryConfig) -> Result<Starlane, HypErr> {
        let artifacts = Artifacts::just_builtins();
        let registry = match config{

            #[cfg(feature = "postgres-embedded")]
            PgRegistryConfig::Embedded(database) => {
                Postgres::new(database).await?;
            }
            PgRegistryConfig::External(database) => {
                /*
                let lookup = PostgresLookups::new();
                let db = lookup.lookup_registry_db()?;
                let mut set = HashSet::new();
                set.insert(db.clone());
                let ctx = Arc::new(PostgresRegistryContext::new(set, Box::new(lookup)).await?);
                let handle = PostgresRegistryContextHandle::new(&db, ctx);
                let postgres_lookups = PostgresLookups::new();

                let logger = root_logger();
                let logger = logger.point(Point::global_registry());
                Arc::new(RegistryWrapper::new(Arc::new(
                    PostgresRegistry::new(handle, Box::new(postgres_lookups), logger).await?,
                )))

                 */
                todo!()
            }
        };

        /*
        Ok(Self {
            registry,
            artifacts,
        })

         */

        todo!()
    }
}

#[async_trait]
impl Platform for Starlane
where
    Self: Sync + Send + Sized,
{
    type Err = HypErr;

    type StarAuth = AnonHyperAuthenticator;
    type RemoteStarConnectionFactory = LocalHyperwayGateJumper;

    fn data_dir(&self) -> String {
        STARLANE_DATA_DIR.clone()
    }

    fn star_auth(&self, star: &StarKey) -> Result<Self::StarAuth, Self::Err> {
        Ok(AnonHyperAuthenticator::new())
    }

    fn remote_connection_factory_for_star(
        &self,
        star: &StarKey,
    ) -> Result<Self::RemoteStarConnectionFactory, Self::Err> {
        todo!()
    }

    fn machine_template(&self) -> MachineTemplate {
        MachineTemplate::default()
    }

    fn machine_name(&self) -> MachineName {
        "singularity".to_string()
    }

    fn drivers_builder(&self, kind: &StarSub) -> DriversBuilder {
        let mut builder = DriversBuilder::new(kind.clone());

        // only allow external Base wrangling external to Super
        if *kind == StarSub::Super {
            builder.add_post(Arc::new(BaseDriverFactory::new(DriverAvail::External)));
        } else {
            builder.add_post(Arc::new(BaseDriverFactory::new(DriverAvail::Internal)));
        }

        match kind {
            StarSub::Central => {
                builder.add_post(Arc::new(RootDriverFactory::new()));
            }
            StarSub::Super => {
                builder.add_post(Arc::new(SpaceDriverFactory::new()));
            }
            StarSub::Nexus => {}
            StarSub::Maelstrom => {
                /*                builder.add_post(Arc::new(HostDriverFactory::new()));
                               builder.add_post(Arc::new(MechtronDriverFactory::new()));

                */
            }
            StarSub::Scribe => {
                /*builder.add_post(Arc::new(RepoDriverFactory::new()));
                builder.add_post(Arc::new(BundleSeriesDriverFactory::new()));
                builder.add_post(Arc::new(BundleDriverFactory::new()));
                builder.add_post(Arc::new(ArtifactDriverFactory::new()));

                 */
            }
            StarSub::Jump => {
                //builder.add_post(Arc::new(WebDriverFactory::new()));
                // builder.add_post(Arc::new(ControlDriverFactory::new()));
            }
            StarSub::Fold => {}
            StarSub::Machine => {
                builder.add_post(Arc::new(ControlDriverFactory::new()));
            }
        }

        builder
    }

    async fn global_registry(&self) -> Result<Registry, Self::Err> {
        Ok(self.registry.clone())
    }

    async fn star_registry(&self, star: &StarKey) -> Result<Registry, Self::Err> {
        todo!()
    }

    fn artifact_hub(&self) -> Artifacts {
        self.artifacts.clone()
    }

    async fn start_services(&self, gate: &Arc<HyperGateSelector>) {
        let dir = match dirs::home_dir() {
            None => ".starlane/localhost/certs".to_string(),
            Some(path) => format!("{}/.starlane/localhost/certs", path.display()),
        };
        fs::create_dir_all(dir.as_str());

        let cert = format!("{}/cert.der", dir.as_str());
        let key = format!("{}/key.der", dir.as_str());
        let cert_path = Path::new(&cert);
        let key_path = Path::new(&key);

        if !cert_path.exists() || !key_path.exists() {
            CertGenerator::gen(vec!["localhost".to_string()])
                .unwrap()
                .write_to_dir(dir.clone())
                .await
                .unwrap();
        };

        let logger = self
            .logger()
            .point(Point::from_str("control-blah").unwrap());
        let server =
            HyperlaneTcpServer::new(STARLANE_CONTROL_PORT.clone(), dir, gate.clone(), logger)
                .await
                .unwrap();
        server.start().unwrap();
    }
}

#[cfg(feature = "postgres")]
#[derive(Clone)]
pub struct PostgresLookups;

#[cfg(feature = "postgres")]
impl PostgresLookups {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(feature = "postgres")]
impl Default for PostgresLookups {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "postgres")]
impl PostgresPlatform for PostgresLookups {
    fn lookup_registry_db(&self) -> Result<Database<PostgresConnectInfo>, RegErr> {
        Ok(Database::from_con(
            STARLANE_REGISTRY_DATABASE.to_string(),
            STARLANE_REGISTRY_SCHEMA.to_string(),
            PostgresConnectInfo::new(
                STARLANE_REGISTRY_URL.to_string(),
                STARLANE_REGISTRY_USER.to_string(),
                STARLANE_REGISTRY_PASSWORD.to_string(),
            ),
        ))
    }

    fn lookup_star_db(&self, star: &StarKey) -> Result<Database<PostgresConnectInfo>, RegErr> {
        Ok(Database::from_con(
            STARLANE_REGISTRY_DATABASE.to_string(),
            star.to_sql_name(),
            PostgresConnectInfo::new(
                STARLANE_REGISTRY_URL.to_string(),
                STARLANE_REGISTRY_USER.to_string(),
                STARLANE_REGISTRY_PASSWORD.to_string(),
            ),
        ))
    }
}
