//pub mod docker;
pub mod err;
pub mod settings;

pub mod kind;

pub mod traits;

pub mod config;

#[derive(Clone, Serialize, Deserialize)]
pub struct StarlaneSettings {
    pub context: String,
    pub home: String,
    pub can_nuke: bool,
    pub can_scorch: bool,
    pub control_port: u16,
    pub foundation: ProtoFoundationSettings,
}

impl StarlaneSettings {}

/*
impl ProtoStarlaneSettings {
    pub fn parse<S>(self) -> Result<StarlaneSettings<S>,FoundationErr> where S: Serialize {
        let rtn = StarlaneSettings {
            context: self.context,
            home: self.home,
            can_nuke: self.can_nuke,
            can_scorch: self.can_scorch,
            control_port: self.control_port,
            foundation: self.foundation.create()?
        };

        Ok(rtn)
    }
}

 */

/*
#[derive(Clone, Serialize, Deserialize)]
pub struct StarlaneSettings<S> where S: Serialize{
    pub context: String,
    pub home: String,
    pub can_nuke: bool,
    pub can_scorch: bool,
    pub control_port: u16,
    #[serde(deserialize_with = "deserialize_from_value")]
    pub foundation: FoundationSettings<S>
}
 */

fn deserialize_from_value<'de, D>(deserializer: D) -> Result<Value, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Deserialize::deserialize(deserializer)?;
    serde_yaml::from_value(value).map_err(de::Error::custom)
}
/*
pub mod traits;
pub mod factory;
pub mod runner;

 */
use crate::hyperspace::foundation::config::{
    DockerDesktopFoundationConfig, FoundationConfig, ProtoFoundationConfig, RawConfig,
};
use crate::hyperspace::foundation::err::FoundationErr;
use crate::hyperspace::foundation::kind::{DependencyKind, FoundationKind, IKind};
use crate::hyperspace::foundation::settings::{
    FoundationSettings, ProtoFoundationSettings, RawSettings,
};
use crate::hyperspace::foundation::traits::{Dependency, Foundation};
use crate::hyperspace::platform::PlatformConfig;
use derive_builder::Builder;
use futures::TryFutureExt;
use itertools::Itertools;
use serde;
use serde::de::{MapAccess, Visitor};
use serde::{de, Deserialize, Deserializer, Serialize};
use serde_yaml::Value;
use std::collections::HashSet;
use std::fmt::{Debug, Display};
use std::future::Future;
use std::hash::Hash;
use std::ops::{Deref, DerefMut};
use std::str::FromStr;

#[derive(Clone)]
pub struct LiveService<S>
where
    S: Clone,
{
    pub service: S,
    tx: tokio::sync::mpsc::Sender<()>,
}

impl<S> Deref for LiveService<S>
where
    S: Clone,
{
    type Target = S;

    fn deref(&self) -> &Self::Target {
        &self.service
    }
}

impl<S> DerefMut for LiveService<S>
where
    S: Clone,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.service
    }
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

    /*
    fn dependency(&self, kind: &DependencyKind) -> Result<impl Dependency, FoundationErr> {
        todo!()
    }

    async fn install_foundation_required_dependencies(&mut self) -> Result<(), FoundationErr> {
        todo!()
    }

     */
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct ProtoFoundationBuilder {
    foundation: FoundationKind,
    config: RawConfig,
    settings: RawSettings,
}

impl ProtoFoundationBuilder {
    pub fn create(self) -> Result<impl Foundation + Sized, FoundationErr> {
        match self.foundation {
            FoundationKind::DockerDesktop => DockerDesktopFoundation::create(self),
        }
    }
}

#[cfg(test)]
pub mod test {
    use crate::hyperspace::foundation::err::FoundationErr;
    use crate::hyperspace::foundation::settings::ProtoFoundationSettings;
    use crate::hyperspace::foundation::{DockerDesktopFoundationSettings, ProtoFoundationBuilder};
    use derive_name::Named;

    #[test]
    pub fn test_builder() {
        fn inner() -> Result<(), FoundationErr> {
            let builder = include_str!("../../../../foundation/docker-desktop.yaml");

            println!("{}", builder);

            let builder = serde_yaml::from_str::<ProtoFoundationBuilder>(builder).unwrap();

            let foundation = builder.create()?;

            Ok(())
        }

        match inner() {
            Ok(_) => {}
            Err(err) => {
                println!("ERR: {}", err);
                assert!(false)
            }
        }
    }
}
