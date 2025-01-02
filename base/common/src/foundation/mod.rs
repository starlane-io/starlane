/// # FOUNDATION
///
///
/// A [crate::Foundation] provides abstracted control over the services and dependencies that drive Starlane.
/// Presently there is only the [DockerDaemonFoundation] which uses a local Docker Service
/// to pull dependent Docker Images, run docker instances and in general enables the Starlane [Platform]
/// manage the lifecycle of arbitrary services.
///
/// A [crate::Foundation] implementation supplies [Provider] implementations each of which have the
/// ability to fetch, download, install, initialize and start external binaries, configs, services,
/// etc. that `Starlane` can incorporate in order to enable new functionality.
///
/// Installing a Postgres Database is a great example since at the time of this writing postgres
/// is required by the Starlane [Registry] and [ProviderKind::PostgresDatabase] is builtin to
/// Starlane.
///
/// Using [DockerDesktopFoundation] as the concrete [crate::Foundation]
///
/// The [`FoundationConfig`] enumerates dependencies which are typically things that don't ship
/// with the Starlane binary.  Common examples are: Postgres, Keycloak, Docker.  Each config
/// core must know how to ready that Dependency and potentially even launch an
/// instance of that Dependency.  For Example: Postgres Database is a base core especially
/// because the default Starlane [`Registry`] (and at the time of this writing the only Registry support).
/// The Postgres [`Dependency`] ensures that Postgres is accessible and properly configured for the
/// Starlane Platform.
///
/// ## THE REGISTRY
/// There is one special core that the Foundation must manage which is the [crate::Foundation::registry]
/// the Starlane Registry is the only required core from the vanilla Starlane installation

use starlane_hyperspace::platform::PlatformConfig;
use downcast_rs::{Downcast, DowncastSync};
use futures::TryFutureExt;
use itertools::Itertools;
use once_cell::sync::Lazy;
use serde;
use serde::de::{MapAccess, Visitor};
use serde::{de, Deserialize, Deserializer, Serialize};
use serde_yaml::Value;
use std::fmt::{Debug, Display};
use std::future::Future;
use std::hash::Hash;
use std::ops::{Deref, DerefMut};
use std::str::FromStr;
use starlane_hyperspace::registry::Registry;
use crate::Foundation;
use crate::provider::Provider;
use crate::kind::{ProviderKind,ProviderKindDef,FoundationKind};


pub mod config;


/// disabled for now ...
//pub mod util;

static REQUIRED: Lazy<Vec<ProviderKindDef>> = Lazy::new(|| vec![]);

pub fn default_requirements() -> Vec<ProviderKindDef> {
    REQUIRED.clone()
}


#[derive(Clone, Serialize, Deserialize)]
pub struct StarlaneConfig {
    pub context: String,
    pub home: String,
    pub can_nuke: bool,
    pub can_scorch: bool,
    pub control_port: u16,
    //    pub foundation: ProtoFoundationSettings,
}

fn deserialize_from_value<'de, D>(deserializer: D) -> Result<Value, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Deserialize::deserialize(deserializer)?;
    serde_yaml::from_value(value).map_err(de::Error::custom)
}

#[derive(Clone)]
pub struct LiveService<K> {
    name: String,
    kind: K,
    tx: tokio::sync::mpsc::Sender<()>,
}

impl<K> LiveService<K> {
    pub fn new(name: String, kind: K, tx: tokio::sync::mpsc::Sender<()>) -> Self {
        Self { name, kind, tx }
    }
}



#[cfg(test)]
pub mod test {}
