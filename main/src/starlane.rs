use serde::{Deserialize, Serialize};
use async_trait::async_trait;
use starlane_hyperspace::driver::{DriverAvail, DriversBuilder};
use starlane_hyperspace::err::HypErr;
use starlane_hyperspace::hyperlane::{AnonHyperAuthenticator, HyperGateSelector, LocalHyperwayGateJumper};
use starlane_hyperspace::machine::MachineTemplate;
use starlane_hyperspace::base::{Platform, PlatformConfig};
use starlane_space::artifact::asynch::Artifacts;
use starlane_space::kind::StarSub;
use starlane_space::loc::{MachineName, StarKey};
use std::sync::Arc;
use starlane_hyperspace::driver::base::BaseDriverFactory;
use starlane_hyperspace::driver::control::ControlDriverFactory;
use starlane_hyperspace::driver::root::RootDriverFactory;
use starlane_hyperspace::driver::space::SpaceDriverFactory;
use std::fs;
use std::path::Path;
use std::str::FromStr;
use port_check::is_local_ipv4_port_free;
use anyhow::anyhow;
use starlane_hyperspace::hyperlane::tcp::{CertGenerator, HyperlaneTcpServer};
use starlane_hyperspace::shutdown::panic_shutdown;
use starlane_macros::push_loc;
use starlane_space::point::Point;
use base::env::STARLANE_CONTROL_PORT;
use hyperspace::registry;


pub mod prelude {
   use starlane_hyperspace::base;
   /// abstract
   pub use base::kinds::Kind;
   pub use base::config::BaseConfig;
   pub use base::config::BaseSubConfig;

   /// [Platform]
   pub use base::Platform;
   pub use base::config::PlatformConfig;
   pub use base::kinds::PlatformKind;

   /// [foundation]
   pub use base::Foundation;
   pub use base::config::FoundationConfig;
   pub use base::kinds::FoundationKind;

   /// [provider]
   pub use base::provider::Provider;
   pub use base::config::ProviderConfig;
   pub use base::kinds::ProviderKind;


    pub mod platform {
        pub mod postgres {
            pub mod service {
                use starlane_platform_for_postgres::service::{Provider,ProviderConfig,PostgresService,PostgresServiceHandle};
            }
            pub mod database {
                use starlane_platform_for_postgres::database::{Provider,ProviderConfig,PostgresService,PostgresDatabaseHandle};
            }



        }
    }

}


mod concrete {
    use crate::starlane::prelude;


    pub struct Platform{
        config: PlatformConfig
    }

    #[derive(Clone,Debug,)]
    pub struct PlatformConfig{
        kind: PlatformKind,
    }

    impl prelude::PlatformConfig for PlatformConfig {
        type RegistryConfig = RegistryConfig;

        fn can_scorch(&self) -> bool {
            todo!()
        }

        fn can_nuke(&self) -> bool {
            todo!()
        }

        fn registry(&self) -> &Self::RegistryConfig {
            todo!()
        }

        fn home(&self) -> &String {
            todo!()
        }

        fn enviro(&self) -> &String {
            todo!()
        }
    }


    #[derive(Clone,Debug,Eq,PartialEq,Hash)]
    pub enum PlatformKind{
       Standalone
    }

    impl prelude::Kind for PlatformKind{ }

    impl prelude::ProviderKind for PlatformKind{ }







}

pub mod config {
    use starlane_hyperspace::base::PlatformConfig;
    use starlane_hyperspace::registry;
    pub trait RegistryConfig:  registry::RegistryConfig { }
}

#[derive(Clone)]
pub struct Starlane {
    artifacts: Artifacts,
    registry: Arc<dyn config::RegistryConfig>
}

impl  Starlane  {
    pub async fn new(
        config: P::Config,
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


mod partial {
    mod my { pub use super::super::*; }

    pub mod config {
        use serde_derive::{Deserialize, Serialize};
        use starlane_hyperspace::registry;
        use super::my;


        /// this [PlatformConfig] is a `partial` because it doesn't have all the necessary
        /// configuration to produce *any* present version of the [registry::Registry].
        /// At the time of this writing the only available registry uses `Postgres` where
        /// this configuration is wrapped with another [PlatformConfig] which then has
        /// all the information it needs to stand up a [registry::Registry] backed by
        /// `Postgres`
        #[derive(Clone, Serialize, Deserialize)]
        pub struct PlatformConfig {
            pub enviro: String,
            pub registry: <Self as starlane_hyperspace::base::PlatformConfig>::RegistryConfig,
            pub home: String,
            pub can_nuke: bool,
            pub can_scorch: bool,
            pub control_port: u16,
        }

        impl <R> my::PlatformConfig for PlatformConfig where R: my::config::RegistryConfig {
            type RegistryConfig = R;

            fn can_scorch(&self) -> bool {
                self.can_scorch
            }

            fn can_nuke(&self) -> bool {
                self.can_nuke
            }

            fn registry(&self) -> &Self::RegistryConfig {
                & self.registry
            }

            fn home(&self) -> &String {
                & self.home
            }

            fn enviro(&self) -> &String {
                & self.enviro
            }
        }




}


}


mod platform {


    pub struct Platform {

    }

    impl Platform
}