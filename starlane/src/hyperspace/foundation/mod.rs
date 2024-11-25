//pub mod docker;
pub mod err;
pub mod settings;

pub mod kind;

pub mod traits;

pub mod config;


pub mod implementation;

pub mod util;

pub mod service;

pub mod dependency;

#[derive(Clone, Serialize, Deserialize)]
pub struct StarlaneConfig {
    pub context: String,
    pub home: String,
    pub can_nuke: bool,
    pub can_scorch: bool,
    pub control_port: u16,
    pub foundation: ProtoFoundationSettings,
}


fn deserialize_from_value<'de, D>(deserializer: D) -> Result<Value, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Deserialize::deserialize(deserializer)?;
    serde_yaml::from_value(value).map_err(de::Error::custom)
}
pub mod factory;
pub mod runner;
use crate::hyperspace::foundation::kind::IKind;
use crate::hyperspace::foundation::settings::ProtoFoundationSettings;
use crate::hyperspace::foundation::traits::{Dependency, Foundation};
use crate::hyperspace::platform::PlatformConfig;
use futures::TryFutureExt;
use itertools::Itertools;
use serde;
use serde::de::{MapAccess, Visitor};
use serde::{de, Deserialize, Deserializer, Serialize};
use serde_yaml::Value;
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




#[cfg(test)]
pub mod test {
    use crate::hyperspace::foundation::err::FoundationErr;

    #[test]
    pub fn test_builder() {
        fn inner() -> Result<(), FoundationErr> {
            let builder = include_str!("../../../../config/foundation/docker-daemon.yaml");

            println!("{}", builder);

            let builder = serde_yaml::from_str::<>(builder).unwrap();

            let foundation = builder.create()?;

            Ok(())
        }

        match inner() {
            Ok(_) => {}
            Err(err) => {
                println!("ERR: {}", err);
                Err::<(),FoundationErr>(err).unwrap();
                assert!(false)
            }
        }
    }
}
