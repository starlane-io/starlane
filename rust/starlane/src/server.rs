use std::collections::HashSet;
use crate::registry::postgres::{
    PostgresDbInfo, PostgresPlatform, PostgresRegistry, PostgresRegistryContext,
    PostgresRegistryContextHandle,
};
use starlane::space::artifact::asynch::Artifacts;
use starlane::space::kind::StarSub;
use starlane::space::loc::{MachineName, StarKey};
use starlane::space::log::RootLogger;
use starlane::space::point::Point;
use std::fs;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use tokio_print::aprintln;
use crate::driver::base::BaseDriverFactory;
use crate::driver::{DriverAvail, DriversBuilder};
use crate::driver::artifact::RepoDriverFactory;
use crate::driver::control::ControlDriverFactory;
use crate::driver::root::RootDriverFactory;
use crate::driver::space::SpaceDriverFactory;
use crate::env::{STARLANE_CONTROL_PORT, STARLANE_DATA_DIR, STARLANE_REGISTRY_DATABASE, STARLANE_REGISTRY_PASSWORD, STARLANE_REGISTRY_URL, STARLANE_REGISTRY_USER};
use crate::err::{HypErr};
use crate::hyperlane::{AnonHyperAuthenticator, HyperGateSelector, LocalHyperwayGateJumper};
use crate::hyperlane::tcp::{CertGenerator, HyperlaneTcpServer};
use crate::hyperspace::machine::MachineTemplate;
use crate::platform::Platform;
use crate::hyperspace::reg::{Registry, RegistryWrapper};
use crate::registry::postgres::err::RegErr;

#[derive(Clone)]
pub struct Starlane {
    pub handle: PostgresRegistryContextHandle, //    pub ctx: P::RegistryContext
    artifacts: Artifacts
}

impl Starlane {
    pub async fn new() -> Result<Starlane, HypErr> {
aprintln!("Starlane::new()");
        #[cfg(feature = "postgres")]
        {
aprintln!("postgres!!!");
            let lookup = StarlanePostgres::new();
            let db = lookup.lookup_registry_db()?;
            let mut set = HashSet::new();
            set.insert(db.clone());
            let ctx = Arc::new(PostgresRegistryContext::new(set,Box::new(lookup)).await?);
            let handle = PostgresRegistryContextHandle::new(&db, ctx);
            let artifacts = Artifacts::just_builtins();
aprintln!("returning postgres handle");
            Ok(Self { handle, artifacts })
        }
        #[cfg(not(feature = "postgres"))]
        {
            let ctx = MemRegCtx::new();
            Ok(Self { ctx })
        }
        /*
        let db = <Self as PostgresPlatform>::lookup_registry_db()?;
        let mut set = HashSet::new();
        set.insert(db.clone());
        let ctx = Arc::new(PostgresRegistryContext::new(set).await?);
        let handle = PostgresRegistryContextHandle::new(&db, ctx);

         */
    }
}

#[async_trait]
impl Platform for Starlane where Self: Sync+Send+Sized{
    type Err = HypErr;
    #[cfg(feature = "postgres")]
    type RegistryContext = PostgresRegistryContextHandle;

    #[cfg(not(feature = "postgres"))]
    type RegistryContext = MemRegCtx;

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
                builder.add_post(Arc::new(RepoDriverFactory::new()));
                /*
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
        let logger = RootLogger::default();
        let logger = logger.point(Point::global_registry());
aprintln!("Creating Global Registry...");
        Ok(Arc::new(RegistryWrapper::new(Arc::new(
            PostgresRegistry::new(self.handle.clone(), Box::new(self.clone()), logger).await?,
        ))))

        //        Ok(Arc::new(MemRegApi::new(self.ctx.clone())))
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


pub struct StarlanePostgres;

impl StarlanePostgres {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(feature = "postgres")]
impl PostgresPlatform for StarlanePostgres{
    fn lookup_registry_db(&self) -> Result<PostgresDbInfo, RegErr> {
        Ok(PostgresDbInfo::new(
            STARLANE_REGISTRY_URL.to_string(),
            STARLANE_REGISTRY_USER.to_string(),
            STARLANE_REGISTRY_PASSWORD.to_string(),
            STARLANE_REGISTRY_DATABASE.to_string(),
        ))
    }

    fn lookup_star_db(&self, star: &StarKey) -> Result<PostgresDbInfo, RegErr> {
        Ok(PostgresDbInfo::new_with_schema(
            STARLANE_REGISTRY_URL.to_string(),
            STARLANE_REGISTRY_USER.to_string(),
            STARLANE_REGISTRY_PASSWORD.to_string(),
            STARLANE_REGISTRY_DATABASE.to_string(),
            star.to_sql_name(),
        ))
    }
}
