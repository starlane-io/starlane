use crate::base::foundation;
use crate::base::foundation::kind::FoundationKind;
use crate::base::foundation::util::{IntoSer, SerMap};
use crate::base::kind::{DependencyKind, IKind, Kind, ProviderKind};
use crate::space::parse::CamelCase;
use derive_name::{Name, Named};
use once_cell::sync::Lazy;
use serde::de::DeserializeOwned;
use serde_derive::{Deserialize, Deserializer, Serialize};
use std::any::Any;
use std::ops::{Deref, DerefMut};
use std::str::FromStr;

pub mod dependency;
/// docker daemon support is supplied through the docker core dependency... there is no need
/// for customizing docker configurations and behavior beyond that...
//pub mod docker;
pub mod postgres;

/// the REQUIRED
static REQUIRED: Lazy<Vec<Kind>> = Lazy::new(|| {
    /// we obviously need DockerDaemon to be installed and running or this Foundation can't do a thing
    let docker_daemon = DependencyKind::DockerDaemon.into();
    /// and this Foundation supports a Postgres Registry (Also very important)
    let registry = ProviderKind::new(
        DependencyKind::PostgresCluster,
        CamelCase::from_str("Registry").unwrap(),
    )
        .into();
    let mut rtn = vec![docker_daemon, registry];

    rtn
});

/// this method is referenced by various [`DependencyConfig`] as a Default value (which in the case of [`FoundationKind::DockerDaemon`] every [`Dependency`]
/// and [`Provisioner`] requires the Docker Daemon to be installed and running
fn default_requirements() -> Vec<Kind> {
    REQUIRED.clone()
}

pub trait Foundation: foundation::Foundation<Config: foundation::skel::FoundationConfig, Dependency: foundation::skel::Dependency, Provider: foundation::skel::Provider> {}
pub trait Dependency: foundation::Dependency<Config: foundation::skel::DependencyConfig, Provider: crate::base::foundation::skel::Provider> {}
pub trait Provider: foundation::Provider<Config: foundation::skel::ProviderConfig> {}

pub trait FoundationConfig: foundation::config::FoundationConfig<DependencyConfig: foundation::skel::DependencyConfig> {}


pub trait DependencyConfig: foundation::config::DependencyConfig {
    /// in this foundation all dependencies are docker images.
    fn image(&self) -> String;
}

pub trait ProviderConfig: foundation::config::ProviderConfig {}

pub mod concrete {
    /// we refer to this as [`my`](my) [`Foundation`] implementation. see [crate::base::foundation::skel] for recommended foundation pattern
    use crate::base;
    use crate::base::err::BaseErr;
    use crate::base::foundation;
    use crate::base::foundation::implementation::docker_daemon_foundation as my;
    use crate::base::foundation::kind::FoundationKind;
    use crate::base::foundation::status::Status;
    use crate::base::kind::{DependencyKind, Kind};
    use crate::space::progress::Progress;
    use derive_name::Name;
    use std::collections::HashMap;
    use std::sync::Arc;


    pub struct Foundation {
        config: Arc<FoundationConfig>,
        status: Arc<tokio::sync::watch::Receiver<Status>>,
        status_tx: tokio::sync::watch::Sender<Status>,
    }

    impl Foundation {
        pub fn new(config: FoundationConfig) -> Self {
            let (status_tx, status) = tokio::sync::watch::channel(Status::default());
            let status = Arc::new(status);
            let config = Arc::new(config);
            Self {
                config,
                status,
                status_tx,
            }
        }
    }

    #[async_trait]
    impl my::Foundation for Foundation {}

    #[async_trait]
    impl foundation::Foundation for Foundation {
        type Config = Arc<FoundationConfig>;

        type Dependency = default::Dependency;
        type Provider = default::Provider;

        fn kind(&self) -> FoundationKind {
            FoundationKind::DockerDaemon
        }

        fn config(&self) -> Self::Config {
            self.config.clone()
        }

        fn status(&self) -> Status {
            self.status.borrow().clone()
        }

        fn status_watcher(&self) -> Arc<tokio::sync::watch::Receiver<Status>> {
            self.status.clone()
        }

        async fn synchronize(&self, progress: Progress) -> Result<Status, BaseErr> {
            todo!();
            Ok(Default::default())
        }
        async fn install(&self, progress: Progress) -> Result<(), BaseErr> {
            Ok(())
        }

        fn dependency(
            &self,
            kind: &DependencyKind,
        ) -> Result<Option<Self::Dependency>, BaseErr> {
            todo!()
        }

        fn registry(&self) -> Result<base::registry::Registry, BaseErr> {
            todo!()
        }
    }


    //#[derive(Clone, Serialize, Deserialize, Name)]
    #[derive(Clone, Name)]
    pub struct FoundationConfig {
        pub kind: FoundationKind,
        //pub registry: RegistryProviderConfig,
        pub dependencies: HashMap<DependencyKind, default::DependencyConfig>,
    }


    pub trait ProviderConfig: base::config::ProviderConfig {}

    impl base::config::FoundationConfig for FoundationConfig {
        type DependencyConfig = default::DependencyConfig;

        fn kind(&self) -> FoundationKind {
            self.kind.clone()
        }

        fn required(&self) -> Vec<Kind> {
            my::default_requirements()
        }

        fn dependency_kinds(&self) -> &Vec<DependencyKind> {
            todo!()
            //        self.dependencies.keys().collect()
        }

        fn dependency(&self, kind: &DependencyKind) -> Option<&Self::DependencyConfig> {
            self.dependencies.get(kind)
        }
    }

    /*
    impl IntoSer for FoundationConfig {
        fn into_ser(&self) -> Box<dyn SerMap> {
            self.clone() as Box<dyn SerMap>
        }
    }

     */


    pub mod default {
        use crate::base::foundation;
        use crate::base::kind::IKind;
        use std::sync::Arc;

        pub type FoundationConfig = super::FoundationConfig;
        pub type DependencyConfig = Arc<dyn super::DependencyConfig<ProviderConfig=ProviderConfig>>;
        pub type ProviderConfig = Arc<dyn super::ProviderConfig>;

        pub type Dependency = Box<dyn super::Dependency<Config=DependencyConfig, Provider=Provider>>;
        pub type Provider = Box<dyn foundation::Provider<Config=ProviderConfig>>;

        /// the defaults are the most concrete implementation of the main traits,
        /// the [`traits`] mod implements the best trait implementation for this foundation suite
        pub mod traits {
            use crate::base::config;
            use crate::base::foundation;

            pub type FoundationConfig = dyn config::FoundationConfig<DependencyConfig=DependencyConfig>;
            pub type DependencyConfig = dyn config::DependencyConfig<ProviderConfig=ProviderConfig>;

            pub type ProviderConfig = dyn config::ProviderConfig;

            pub type Foundation<D, P> = dyn foundation::Foundation<Config=FoundationConfig, Dependency=D, Provider=P>;
            pub type Dependency = dyn foundation::Dependency<Config=DependencyConfig, Provider=Provider>;
            pub type Provider = Box<dyn foundation::Provider<Config=ProviderConfig>>;
        }
    }
}


#[cfg(test)]
pub mod test {
    use crate::base::err::BaseErr;
    #[test]
    pub fn foundation_config() {
        fn inner() -> Result<(), BaseErr> {
            let foundation_config = include_str!("docker-daemon-test-config.yaml");
            let foundation_config =
                serde_yaml::from_str(foundation_config).map_err(BaseErr::config_err)?;

            todo!();
            //            let foundation_config = FoundationConfig::des_from_map(foundation_config)?;

            Ok(())
        }

        match inner() {
            Ok(_) => {}
            Err(err) => {
                println!("ERR: {}", err);
                Err::<(), BaseErr>(err).unwrap();
                assert!(false)
            }
        }
    }
}
