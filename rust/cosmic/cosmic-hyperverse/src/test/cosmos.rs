use crate::driver::artifact::{
    ArtifactDriverFactory, BundleDriverFactory, BundleSeriesDriverFactory, RepoDriverFactory,
};
use crate::driver::base::BaseDriverFactory;
use crate::driver::control::ControlDriverFactory;
use crate::driver::root::RootDriverFactory;
use crate::driver::space::SpaceDriverFactory;
use crate::driver::DriverAvail;
use crate::test::registry::{TestRegistryApi, TestRegistryContext};
use crate::{DriversBuilder, HyperErr, Cosmos, MachineTemplate, Registry};
use cosmic_hyperlane::{AnonHyperAuthenticator, HyperGate, LocalHyperwayGateJumper};
use cosmic_universe::artifact::{ArtifactApi, ReadArtifactFetcher};
use cosmic_universe::err::UniErr;
use cosmic_universe::kind::{BaseKind, Kind, StarSub};
use cosmic_universe::loc::{MachineName, StarKey, ToBaseKind};
use cosmic_universe::particle::property::{PropertiesConfig, PropertiesConfigBuilder};
use std::io;
use std::io::Error;
use std::sync::Arc;
use tokio::sync::oneshot;
use tokio::sync::oneshot::error::RecvError;
use tokio::time::error::Elapsed;
use mechtron_host::err::HostErr;
use crate::driver::mechtron::{HostDriverFactory, MechtronDriverFactory};

impl TestCosmos {
    pub fn new() -> Self {
        Self {
            ctx: TestRegistryContext::new(),
        }
    }
}

#[derive(Clone)]
pub struct TestCosmos {
    pub ctx: TestRegistryContext,
}

#[async_trait]
impl Cosmos for TestCosmos {
    type Err = TestErr;
    type RegistryContext = TestRegistryContext;
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
        "test".to_string()
    }

    fn properties_config(&self, kind: &Kind) -> PropertiesConfig {
        let mut builder = PropertiesConfigBuilder::new();
        builder.kind(kind.clone());
        match kind.to_base() {
            BaseKind::Mechtron => {
                builder.add_point( "config", true, true ).unwrap();
                builder.build().unwrap()
            }
            _ => {
                builder.build().unwrap()
            }
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
                builder.add_post(Arc::new(ControlDriverFactory::new()));
            }
            StarSub::Fold => {}
            StarSub::Machine => {
                builder.add_post(Arc::new(ControlDriverFactory::new()));
            }
        }

        builder
    }

    async fn global_registry(&self) -> Result<Registry<Self>, Self::Err> {
        Ok(Arc::new(TestRegistryApi::new(self.ctx.clone())))
    }

    async fn star_registry(&self, star: &StarKey) -> Result<Registry<Self>, Self::Err> {
        todo!()
    }

    fn artifact_hub(&self) -> ArtifactApi {
        ArtifactApi::no_fetcher()
    }

    /*
    fn artifact_hub(&self) -> ArtifactApi {
        ArtifactApi::new(Arc::new(ReadArtifactFetcher::new()))
    }

     */

    fn start_services(&self, gate: &Arc<dyn HyperGate>) {}
}

#[derive(Debug, Clone)]
pub struct TestErr {
    pub message: String,
}

impl TestErr {
    pub fn new<S: ToString>(message: S) -> Self {
        Self {
            message: message.to_string(),
        }
    }
}

impl ToString for TestErr {
    fn to_string(&self) -> String {
        self.message.clone()
    }
}

impl Into<UniErr> for TestErr {
    fn into(self) -> UniErr {
        UniErr::from_500(self.to_string())
    }
}

impl From<oneshot::error::RecvError> for TestErr {
    fn from(err: RecvError) -> Self {
        TestErr {
            message: err.to_string(),
        }
    }
}

impl From<Elapsed> for TestErr {
    fn from(err: Elapsed) -> Self {
        TestErr {
            message: err.to_string(),
        }
    }
}

impl From<String> for TestErr {
    fn from(err: String) -> Self {
        TestErr { message: err }
    }
}

impl From<&'static str> for TestErr {
    fn from(err: &'static str) -> Self {
        TestErr {
            message: err.to_string(),
        }
    }
}

impl From<UniErr> for TestErr {
    fn from(err: UniErr) -> Self {
        Self {
            message: err.to_string(),
        }
    }
}

impl From<io::Error> for TestErr {
    fn from(err: Error) -> Self {
        Self {
            message: err.to_string(),
        }
    }
}

impl From<HostErr> for TestErr {
    fn from(err: HostErr) -> Self {
        Self {
            message: err.to_string(),
        }
    }
}
impl From<acid_store::Error> for TestErr {
    fn from(e: acid_store::Error) -> Self {
        Self {
            message: e.to_string(),
        }
    }
}

impl From<zip::result::ZipError> for TestErr {
    fn from(a: zip::result::ZipError) -> Self {
        Self {
            message: a.to_string(),
        }
    }
}

impl From<Box<bincode::ErrorKind>> for TestErr {
    fn from(e: Box<bincode::ErrorKind>) -> Self {
        Self {
            message: e.to_string(),
        }
    }
}

impl HyperErr for TestErr {
    fn to_cosmic_err(&self) -> UniErr {
        UniErr::from_500(self.to_string())
    }

    fn new<S>(message: S) -> Self
    where
        S: ToString,
    {
        Self {
            message: message.to_string(),
        }
    }

    fn status_msg<S>(status: u16, message: S) -> Self
    where
        S: ToString,
    {
        Self {
            message: message.to_string(),
        }
    }

    fn status(&self) -> u16 {
        500u16
    }
}
