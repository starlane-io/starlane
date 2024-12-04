use crate::base::foundation::err::FoundationErr;
use crate::base::foundation::implementation::docker_daemon_foundation;
use crate::base::foundation::util::{IntoSer, Map, SerMap};
use crate::space::parse::CamelCase;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use crate::base;
use crate::base::kind::{DependencyKind, Kind, ProviderKind};

static REQUIRED: Lazy<Vec<Kind>> = Lazy::new(|| {
    let mut rtn = vec![];

    rtn
});

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DockerDaemonCoreDependencyConfig {
    providers: HashMap<CamelCase, Arc<DockerProviderCoreConfig>>,
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

impl base::config::DependencyConfig for DockerDaemonCoreDependencyConfig {
    fn kind(&self) -> &DependencyKind {
        &DependencyKind::DockerDaemon
    }

    fn volumes(&self) -> HashMap<String, String> {
        Default::default()
    }

    fn require(&self) -> Vec<Kind> {
        REQUIRED.clone()
    }


}

impl IntoSer for DockerDaemonCoreDependencyConfig {
    fn into_ser(&self) -> Box<dyn SerMap> {
        todo!()
        //self.clone() as Box<dyn SerMap>
    }
}

impl base::config::ProviderConfigSrc for DockerDaemonCoreDependencyConfig {
    type Config = Arc<DockerProviderCoreConfig>;

    fn providers(&self) -> Result<HashMap<CamelCase, Self::Config>, FoundationErr> {
        Ok(self.providers.clone())
    }

    fn provider(&self, kind: &CamelCase) -> Result<Option<&Self::Config>, FoundationErr> {
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

pub trait ProviderConfig: base::config::ProviderConfig {
    fn image(&self) -> String;

    fn expose(&self) -> HashMap<u16, u16>;
}

impl base::config::ProviderConfig for DockerProviderCoreConfig {
    fn kind(&self) -> &ProviderKind {
        &self.kind
    }

    fn clone_me(&self) -> Arc<dyn base::config::ProviderConfig> {
        Arc::new(self.clone())
    }
}
impl IntoSer for DockerProviderCoreConfig {
    fn into_ser(&self) -> Box<dyn SerMap> {
        todo!();
        //self.clone() as Box<dyn SerMap>
    }
}
