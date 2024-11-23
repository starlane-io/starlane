use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::hyperspace::foundation::config::FoundationConfig;
use crate::hyperspace::foundation::err::FoundationErr;
use crate::hyperspace::foundation::kind::{DependencyKind, FoundationKind};
use crate::hyperspace::foundation::ProtoFoundationBuilder;
use crate::hyperspace::foundation::settings::FoundationSettings;
use crate::hyperspace::foundation::traits::Foundation;

pub mod dependency;

#[derive(Debug,Clone,Eq,PartialEq,Serialize,Deserialize)]
pub struct DockerDesktopDependencyConfig {
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    image: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    compose: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    volumes: HashMap<String,String>
}

#[derive(Debug, Clone,Serialize,Deserialize,Eq,PartialEq)]
pub struct DockerDesktopFoundationConfig {
   pub dependencies: HashMap<DependencyKind,DockerDesktopDependencyConfig>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct DockerDesktopFoundationSettings {}

impl DockerDesktopFoundationSettings {
    pub fn new(name: String) -> Self {
        Self {}
    }
}

pub struct DockerDesktopFoundation {
    config: FoundationConfig<DockerDesktopFoundationConfig>,
    settings: FoundationSettings<DockerDesktopFoundationSettings>,
}

impl Foundation for DockerDesktopFoundation {
    fn create(builder: ProtoFoundationBuilder) -> Result<impl Foundation + Sized, FoundationErr> {

        let config = FoundationConfig::new(FoundationKind::DockerDesktop, serde_yaml::from_value(builder.config).map_err(FoundationErr::config_err)?);
        let settings = FoundationSettings::new(FoundationKind::DockerDesktop, serde_yaml::from_value(builder.settings).map_err(FoundationErr::settings_err)?);

        Ok(Self { settings, config })
    }

    fn foundation_kind() -> FoundationKind {
        FoundationKind::DockerDesktop
    }

}