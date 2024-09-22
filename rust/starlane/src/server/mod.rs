use crate::err::StarErr;
use std::collections::HashSet;

use crate::driver::base::BaseDriverFactory;
use crate::driver::control::ControlDriverFactory;
use crate::driver::root::RootDriverFactory;
use crate::driver::space::SpaceDriverFactory;
use crate::driver::{DriverAvail, DriversBuilder};
use crate::env::{
    STARLANE_CONTROL_PORT, STARLANE_DATA_DIR, STARLANE_REGISTRY_DATABASE,
    STARLANE_REGISTRY_PASSWORD, STARLANE_REGISTRY_URL, STARLANE_REGISTRY_USER,
};
use crate::hyper::lane::tcp::{CertGenerator, HyperlaneTcpServer};
use crate::hyper::lane::{AnonHyperAuthenticator, HyperGateSelector, LocalHyperwayGateJumper};
use crate::hyper::space::machine::MachineTemplate;
use crate::hyper::space::platform::Platform;
use crate::hyper::space::reg::{Registry, RegistryWrapper};
use crate::registry::postgres::{
    PostgresDbInfo, PostgresPlatform, PostgresRegistry, PostgresRegistryContext,
    PostgresRegistryContextHandle,
};
use starlane_space::artifact::asynch::ArtifactApi;
use starlane_space::kind::StarSub;
use starlane_space::loc::{MachineName, StarKey};
use starlane_space::log::RootLogger;
use starlane_space::point::Point;
use std::fs;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
//use crate::driver::artifact::RepoDriverFactory;

#[derive(Clone)]
pub struct Starlane {
    pub handle: PostgresRegistryContextHandle<Self>, //    pub ctx: P::RegistryContext
}

impl Starlane {
    pub async fn new() -> Result<Starlane, StarErr> {
        #[cfg(feature = "postgres")]
        {
            let db = <Self as PostgresPlatform>::lookup_registry_db()?;
            let mut set = HashSet::new();
            set.insert(db.clone());
            let ctx = Arc::new(PostgresRegistryContext::new(set).await?);
            let handle = PostgresRegistryContextHandle::new(&db, ctx);
            Ok(Self { handle })
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
impl Platform for Starlane {
    type Err = StarErr;
    #[cfg(feature = "postgres")]
    type RegistryContext = PostgresRegistryContextHandle<Self>;

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

    fn drivers_builder(&self, kind: &StarSub) -> DriversBuilder<Self> {
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
                /*
                builder.add_post(Arc::new(RepoDriverFactory::new()));
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

    async fn global_registry(&self) -> Result<Registry<Self>, Self::Err> {
        let logger = RootLogger::default();
        let logger = logger.point(Point::global_registry());
        Ok(Arc::new(RegistryWrapper::new(Arc::new(
            PostgresRegistry::new(self.handle.clone(), self.clone(), logger).await?,
        ))))

        //        Ok(Arc::new(MemRegApi::new(self.ctx.clone())))
    }

    async fn star_registry(&self, star: &StarKey) -> Result<Registry<Self>, Self::Err> {
        todo!()
    }

    fn artifact_hub(&self) -> ArtifactApi {
        ArtifactApi::no_fetcher()
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
            .point(Point::from_str("control-server").unwrap());
        let server =
            HyperlaneTcpServer::new(STARLANE_CONTROL_PORT.clone(), dir, gate.clone(), logger)
                .await
                .unwrap();
        server.start().unwrap();
    }
}

#[cfg(feature = "postgres")]
impl PostgresPlatform for Starlane {
    fn lookup_registry_db() -> Result<PostgresDbInfo, Self::Err> {
        Ok(PostgresDbInfo::new(
            STARLANE_REGISTRY_URL.to_string(),
            STARLANE_REGISTRY_USER.to_string(),
            STARLANE_REGISTRY_PASSWORD.to_string(),
            STARLANE_REGISTRY_DATABASE.to_string(),
        ))
    }

    fn lookup_star_db(star: &StarKey) -> Result<PostgresDbInfo, Self::Err> {
        Ok(PostgresDbInfo::new_with_schema(
            STARLANE_REGISTRY_URL.to_string(),
            STARLANE_REGISTRY_USER.to_string(),
            STARLANE_REGISTRY_PASSWORD.to_string(),
            STARLANE_REGISTRY_DATABASE.to_string(),
            star.to_sql_name(),
        ))
    }
}
