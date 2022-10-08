#![allow(warnings)]
use std::fs;
use cosmic_hyperlane_tcp::CertGenerator;
pub mod err;
pub mod properties;

#[macro_use]
extern crate async_trait;
#[macro_use]
extern crate lazy_static;

use std::collections::HashSet;

use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use tokio::io;
use tokio::runtime::Runtime;
use uuid::Uuid;

use crate::err::StarErr;
use cosmic_hyperlane::{
    AnonHyperAuthenticator, HyperGate, HyperGateSelector, LocalHyperwayGateJumper,
};
use cosmic_hyperverse::driver::artifact::{
    ArtifactDriverFactory, BundleDriverFactory, BundleSeriesDriverFactory, RepoDriverFactory,
};
use cosmic_hyperverse::driver::base::BaseDriverFactory;
use cosmic_hyperverse::driver::control::ControlDriverFactory;
use cosmic_hyperverse::driver::mechtron::{HostDriverFactory, MechtronDriverFactory};
use cosmic_hyperverse::driver::root::RootDriverFactory;
use cosmic_hyperverse::driver::space::SpaceDriverFactory;
use cosmic_hyperverse::driver::{DriverAvail, DriversBuilder};
use cosmic_hyperverse::err::CosmicErr;
use cosmic_hyperverse::machine::{Machine, MachineTemplate};
use cosmic_hyperverse::reg::{Registry, RegistryApi};
use cosmic_hyperverse::Cosmos;
use cosmic_registry_postgres::err::PostErr;
use cosmic_registry_postgres::{
    PostgresDbInfo, PostgresPlatform, PostgresRegistry, PostgresRegistryContext,
    PostgresRegistryContextHandle,
};
use cosmic_universe::artifact::ArtifactApi;
use cosmic_universe::artifact::ReadArtifactFetcher;
use cosmic_universe::command::direct::create::KindTemplate;
use cosmic_universe::err::UniErr;
use cosmic_universe::kind::{
    ArtifactSubKind, BaseKind, FileSubKind, Kind, Specific, StarSub, UserBaseSubKind,
};
use cosmic_universe::loc::{MachineName, StarKey};
use cosmic_universe::loc::{Point, ToBaseKind};
use cosmic_universe::log::RootLogger;
use cosmic_universe::particle::property::{
    AnythingPattern, BoolPattern, EmailPattern, PointPattern, PropertiesConfig,
    PropertiesConfigBuilder, PropertyPermit, PropertySource, U64Pattern, UsernamePattern,
};
use cosmic_universe::substance::Token;

use cosmic_hyperlane_tcp::HyperlaneTcpServer;

fn main() -> Result<(), StarErr> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async move {
        let starlane = Starlane::new().await.unwrap();
        let machine_api = starlane.machine();
        tokio::time::timeout(Duration::from_secs(30), machine_api.wait_ready())
            .await
            .unwrap();
println!("> STARLANE Ready!");
        // this is a dirty hack which is good enough for a 0.3.0 release...
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
        let cl = machine_api.clone();
        machine_api.await_termination().await.unwrap();
        cl.terminate();
    });
    Ok(())
}

lazy_static! {
    pub static ref STARLANE_CONTROL_PORT: u16 = std::env::var("STARLANE_PORT")
        .unwrap_or("4343".to_string())
        .parse::<u16>()
        .unwrap_or(4343);
    pub static ref STARLANE_DATA_DIR: String =
        std::env::var("STARLANE_DATA_DIR").unwrap_or("data".to_string());
    pub static ref STARLANE_CACHE_DIR: String =
        std::env::var("STARLANE_CACHE_DIR").unwrap_or("cache".to_string());
    pub static ref STARLANE_TOKEN: String =
        std::env::var("STARLANE_TOKEN").unwrap_or(Uuid::new_v4().to_string());
    pub static ref STARLANE_REGISTRY_URL: String =
        std::env::var("STARLANE_REGISTRY_URL").unwrap_or("localhost".to_string());
    pub static ref STARLANE_REGISTRY_USER: String =
        std::env::var("STARLANE_REGISTRY_USER").unwrap_or("postgres".to_string());
    pub static ref STARLANE_REGISTRY_PASSWORD: String =
        std::env::var("STARLANE_REGISTRY_PASSWORD").unwrap_or("password".to_string());
    pub static ref STARLANE_REGISTRY_DATABASE: String =
        std::env::var("STARLANE_REGISTRY_DATABASE").unwrap_or("postgres".to_string());
    pub static ref STARLANE_CERTS_DIR: String =
        std::env::var("STARLANE_CERTS_DIR").unwrap_or("./certs".to_string());
}
#[no_mangle]
pub extern "C" fn cosmic_uuid() -> String {
    Uuid::new_v4().to_string()
}

#[no_mangle]
pub extern "C" fn cosmic_timestamp() -> DateTime<Utc> {
    Utc::now()
}

#[derive(Clone)]
pub struct Starlane {
    pub handle: PostgresRegistryContextHandle<Self>,
}

impl Starlane {
    pub async fn new() -> Result<Self, StarErr> {
        let db = <Self as PostgresPlatform>::lookup_registry_db()?;
        let mut set = HashSet::new();
        set.insert(db.clone());
        let ctx = Arc::new(PostgresRegistryContext::new(set).await?);
        let handle = PostgresRegistryContextHandle::new(&db, ctx);
        Ok(Self { handle })
    }
}

#[async_trait]
impl Cosmos for Starlane {
    type Err = StarErr;
    type RegistryContext = PostgresRegistryContextHandle<Self>;
    type StarAuth = AnonHyperAuthenticator;
    type RemoteStarConnectionFactory = LocalHyperwayGateJumper;

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
        "starlane".to_string()
    }

    fn properties_config(&self, kind: &Kind) -> PropertiesConfig {
        let mut builder = PropertiesConfigBuilder::new();
        builder.kind(kind.clone());
        match kind.to_base() {
            BaseKind::Mechtron => {
                builder.add_point("config", true, true).unwrap();
                builder.build().unwrap()
            }
            _ => builder.build().unwrap(),
        }
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
                builder.add_post(Arc::new(HostDriverFactory::new()));
                builder.add_post(Arc::new(MechtronDriverFactory::new()));
            }
            StarSub::Scribe => {
                builder.add_post(Arc::new(RepoDriverFactory::new()));
                builder.add_post(Arc::new(BundleSeriesDriverFactory::new()));
                builder.add_post(Arc::new(BundleDriverFactory::new()));
                builder.add_post(Arc::new(ArtifactDriverFactory::new()));
            }
            StarSub::Jump => {
                //                builder.add_post(Arc::new(ControlDriverFactory::new()));
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
        Ok(Arc::new(
            PostgresRegistry::new(self.handle.clone(), self.clone(), logger).await?,
        ))
    }

    async fn star_registry(&self, star: &StarKey) -> Result<Registry<Self>, Self::Err> {
        todo!()
    }

    fn artifact_hub(&self) -> ArtifactApi {
        ArtifactApi::no_fetcher()
    }

    async fn start_services(&self, gate: &Arc<HyperGateSelector>) {
        fs::create_dir_all(STARLANE_CERTS_DIR.as_str() );
        let cert = format!("{}/cert.pem", STARLANE_CERTS_DIR.as_str());
        let key = format!("{}/key.pem", STARLANE_CERTS_DIR.as_str());
        let cert_path =  Path::new(&cert );
        let key_path=  Path::new(&key);

        if !cert_path.exists() || !key_path.exists() {
            CertGenerator::gen(vec!["localhost".to_string()]).unwrap()
            .write_to_dir(STARLANE_CERTS_DIR.clone())
            .await.unwrap();
        };

        let logger = self.logger().point(Point::from_str("control-server").unwrap());
        let server =
            HyperlaneTcpServer::new(STARLANE_CONTROL_PORT.clone(), STARLANE_CERTS_DIR.clone(), gate.clone(), logger)
                .await.unwrap();
        server.start().unwrap();
    }
}

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
