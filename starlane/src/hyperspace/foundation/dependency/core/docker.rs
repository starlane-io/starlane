use crate::hyperspace::foundation::config;
use crate::hyperspace::foundation::dependency::core::postgres::PostgresClusterCoreConfig;
use crate::hyperspace::foundation::err::FoundationErr;
use crate::hyperspace::foundation::implementation::docker_daemon_foundation;
use crate::hyperspace::foundation::kind::{DependencyKind, Kind, ProviderKind};
use crate::hyperspace::foundation::util::{IntoSer, Map, SerMap};
use crate::space::parse::CamelCase;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

static REQUIRED: Lazy<Vec<Kind>> = Lazy::new(|| {
    let mut rtn = vec![];

    rtn
});

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DockerDaemonCoreDependencyConfig {
    providers: HashMap<CamelCase, DockerProviderCoreConfig>,
}

impl DockerDaemonCoreDependencyConfig {
    pub fn create(config: Map) -> Result<Self, FoundationErr> {
        let providers = config.parse_same("providers")?;

        Ok(DockerDaemonCoreDependencyConfig { providers })
    }
    pub fn create_as_trait(
        config: Map,
    ) -> Result<Arc<dyn docker_daemon_foundation::DependencyConfig>, FoundationErr> {
        Ok(Self::create(config)?.into_trait())
    }

    pub fn into_trait(self) -> Arc<dyn docker_daemon_foundation::DependencyConfig> {
        let config = Arc::new(self);
        config as Arc<dyn docker_daemon_foundation::DependencyConfig>
    }
}

impl docker_daemon_foundation::DependencyConfig for DockerDaemonCoreDependencyConfig {
    fn image(&self) -> String {
        todo!()
    }
}

impl config::DependencyConfig for DockerDaemonCoreDependencyConfig {
    fn kind(&self) -> &DependencyKind {
        &DependencyKind::DockerDaemon
    }

    fn volumes(&self) -> HashMap<String, String> {
        Default::default()
    }

    fn require(&self) -> Vec<Kind> {
        REQUIRED.clone()
    }

    fn clone_me(&self) -> Arc<dyn config::DependencyConfig> {
        Arc::new(self.clone())
    }
}

impl IntoSer for DockerDaemonCoreDependencyConfig {
    fn into_ser(&self) -> Box<dyn SerMap> {
        self.clone() as Box<dyn SerMap>
    }
}

impl config::ProviderConfigSrc<DockerProviderCoreConfig> for DockerDaemonCoreDependencyConfig {
    fn providers(&self) -> Result<&HashMap<CamelCase, DockerProviderCoreConfig>, FoundationErr> {
        Ok(&self.providers)
    }

    fn provider(
        &self,
        kind: &CamelCase,
    ) -> Result<Option<&DockerProviderCoreConfig>, FoundationErr> {
        Ok(self.providers.get(kind))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerProviderCoreConfig {
    kind: ProviderKind,
    image: String,
    expose: HashMap<u16, u16>,
}

impl ProviderConfig for DockerProviderCoreConfig {
    fn image(&self) -> String {
        self.image.clone()
    }

    fn expose(&self) -> HashMap<u16, u16> {
        self.expose.clone()
    }
}

impl DockerProviderCoreConfig {
    pub fn new(
        kind: ProviderKind,
        image: String,
        expose: HashMap<u16, u16>,
    ) -> DockerProviderCoreConfig {
        Self {
            kind,
            image,
            expose,
        }
    }

    pub fn create(config: Map) -> Result<Self, FoundationErr> {
        let kind: CamelCase = config
            .from_field("kind")
            .map_err(FoundationErr::config_err)?;
        let kind = ProviderKind::new(DependencyKind::DockerDaemon, kind);
        let image = config
            .from_field("image")
            .map_err(FoundationErr::config_err)?;
        let expose = config
            .from_field_opt("expose")
            .map_err(FoundationErr::config_err)?;

        let expose = match expose {
            Some(expose) => expose,
            None => HashMap::new(),
        };

        Ok(Self {
            kind,
            image,
            expose,
        })
    }
}

pub trait ProviderConfig: config::ProviderConfig {
    fn image(&self) -> String;

    fn expose(&self) -> HashMap<u16, u16>;
}

impl config::ProviderConfig for DockerProviderCoreConfig {
    fn kind(&self) -> &ProviderKind {
        &self.kind
    }

    fn clone_me(&self) -> Arc<dyn config::ProviderConfig> {
        Arc::new(self.clone())
    }
}
impl IntoSer for DockerProviderCoreConfig {
    fn into_ser(&self) -> Box<dyn SerMap> {
        self.clone() as Box<dyn SerMap>
    }
}
