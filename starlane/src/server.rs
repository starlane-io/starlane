#[cfg(feature = "postgres")]
use crate::hyperspace::registry::postgres::{
    PostgresConnectInfo, PostgresPlatform, PostgresRegistry, PostgresRegistryContext,
    PostgresRegistryContextHandle,
};

use crate::hyperspace::reg::Registry;
use crate::hyperspace::driver::base::BaseDriverFactory;
use crate::hyperspace::driver::control::ControlDriverFactory;
use crate::hyperspace::driver::root::RootDriverFactory;
use crate::hyperspace::driver::{DriverAvail, DriversBuilder};
use crate::space::artifact::asynch::Artifacts;
use crate::space::kind::StarSub;
use crate::space::loc::{MachineName, StarKey};
use crate::space::point::Point;
use std::fs;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

use crate::base::foundation::Foundation;
use crate::env::{config_path, STARLANE_CONTROL_PORT, STARLANE_DATA_DIR, STARLANE_HOME};
use crate::hyperspace::driver::space::SpaceDriverFactory;
use crate::hyperspace::err::HypErr;
use crate::hyperspace::hyperlane::tcp::{CertGenerator, HyperlaneTcpServer};
use crate::hyperspace::hyperlane::{
    AnonHyperAuthenticator, HyperGateSelector, LocalHyperwayGateJumper,
};
//use crate::hyperspace::config::docker::DockerDesktopFoundation;
use crate::hyperspace::machine::MachineTemplate;
use crate::hyperspace::platform::{Platform, PlatformConfig};
use crate::hyperspace::registry::err::RegErr;
use crate::hyperspace::shutdown::panic_shutdown;
use anyhow::anyhow;
use port_check::is_local_ipv4_port_free;
use serde::{Deserialize, Serialize};
use starlane_primitive_macros::{logger, push_loc};
use std::collections::HashSet;
use std::ops::Deref;
use wasmer_wasix::virtual_net::VirtualConnectedSocketExt;
use crate::hyperspace::reg::RegistryApi;

#[derive(Clone, Serialize, Deserialize)]
pub struct StarlaneConfig {
    pub context: String,
    pub home: String,
    pub can_nuke: bool,
    pub can_scorch: bool,
    pub control_port: u16,
    //    pub config: ProtoFoundationConfig,
    pub registry: (),
}

impl PlatformConfig for StarlaneConfig {
    type RegistryConfig = ();

    fn can_scorch(&self) -> bool {
        self.can_scorch
    }

    fn can_nuke(&self) -> bool {
        self.can_nuke
    }

    fn registry(&self) -> &Self::RegistryConfig {
        &self.registry
    }

    fn home(&self) -> &String {
        &self.home
    }

    fn data_dir(&self) -> &String {
        todo!()
    }
}

impl Default for StarlaneConfig {
    fn default() -> StarlaneConfig {
        todo!()
        /*
        Self {
            context: "default".to_string(),
            home: STARLANE_HOME.to_string(),
            can_nuke: false,
            can_scorch: false,
            control_port: 4343u16,
            registry: PgRegistryConfig::default(),
        }

         */
    }
}

#[derive(Clone)]
pub struct Starlane {
    config: StarlaneConfig,
    artifacts: Artifacts,
    registry: Arc<dyn RegistryApi>
    //    config: DockerDesktopFoundation,
}

/*
impl Into<Database<PostgresConnectInfo>> for Database<PgEmbedSettings> {
    fn into(self) -> Database<PostgresConnectInfo> {
        Database {
            settings: PostgresConnectInfo {
                url: "localhost".to_string(),
                user: self.user.clone(),
                password: self.password.clone()
            },
            database: self.database,
            schema: self.schema,
        }
    }
}

 */

#[cfg(feature = "postgres")]
#[cfg(feature = "blah")]
impl Starlane {
    pub async fn new(
        config: StarlaneConfig,
        foundation: DockerDesktopFoundation,
    ) -> Result<Starlane, HypErr> {
        todo!();
        /*
        let artifacts = Artifacts::just_builtins();

        let db = match config.clone().registry {
            PgRegistryConfig::Embedded(db) => {
                let rtn = config.provision_registry(&config).await?;
                rtn
            }
            PgRegistryConfig::External(db) => {
                let (handle, mut rx) = tokio::sync::mpsc::channel(1);
                tokio::spawn(async move {
                    while let Some(_) = rx.recv().await {
                        // do nothing until sender goes out of scope
                    }
                });

                LiveDatabase::new(db, handle)
            }
        };

        let lookups = PostgresLookups::new(config.registry.clone());
        let mut set = HashSet::new();
        set.insert(db.database.clone());
        let ctx = Arc::new(PostgresRegistryContext::new(set, Box::new(lookups.clone())).await?);
        let handle = PostgresRegistryContextHandle::new(&db.database, ctx, db.handle);

        let logger = logger!(&Point::global_registry());

        let registry = Arc::new(RegistryWrapper::new(Arc::new(
            PostgresRegistry::new(handle, Box::new(lookups), logger).await?,
        )));

        Ok(Self {
            config,
            registry,
            artifacts,
            config,
        })

         */
    }
}

impl Drop for Starlane {
    fn drop(&mut self) {
        match &self.config.registry {
            PgRegistryConfig::Embedded(db) => {}
            _ => {}
        };
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

    type Foundation = ();

    type Config = StarlaneConfig;

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

        let logger = push_loc!((self.logger(), Point::from_str("control-blah").unwrap()));

        if !is_local_ipv4_port_free(STARLANE_CONTROL_PORT.clone()) {
            panic_shutdown(format!(
                "starlane port '{}' is being used by another process",
                STARLANE_CONTROL_PORT.to_string()
            ));
        }

        let server =
            HyperlaneTcpServer::new(STARLANE_CONTROL_PORT.clone(), dir, gate.clone(), logger)
                .await
                .unwrap();
        server.start().unwrap();
    }

    fn config(&self) -> &Self::Config {
        &self.config
    }

    async fn scorch(&self) -> Result<(), Self::Err> {
        if !self.config().can_scorch() {
            Err(anyhow!("in config '{}' can_scorch=false", config_path()))?;
        }
        self.global_registry().await.unwrap().scorch().await?;
        Ok(())
    }
}

#[cfg(feature = "postgres")]
#[derive(Clone)]
pub struct PostgresLookups(LiveDatabase);

#[cfg(feature = "postgres")]
impl PostgresLookups {
    pub fn new(database: LiveDatabase) -> Self {
        Self(database)
    }
}

/*
#[cfg(feature = "postgres")]
impl Default for PostgresLookups {
    fn default() -> Self {
        Self::new(PgR)
    }
}

 */

#[cfg(feature = "postgres")]
impl PostgresPlatform for PostgresLookups {
    fn lookup_registry_db(&self) -> Result<Database<PostgresConnectInfo>, RegErr> {
        Ok(self.0.clone().into())
    }

    fn lookup_star_db(&self, star: &StarKey) -> Result<Database<PostgresConnectInfo>, RegErr> {
        let mut rtn: Database<PostgresConnectInfo> = self.0.clone().into();
        rtn.database = star.to_sql_name();
        Ok(rtn)
    }
}

pub struct StarlaneContext {
    pub context: String,
    pub home: String,
    pub log_dir: String,
    pub config: StarlaneConfig,
}
