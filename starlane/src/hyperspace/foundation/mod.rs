//pub mod docker;
pub mod config;
pub mod err;

pub mod kind;


#[derive(Clone, Serialize, Deserialize)]
pub struct StarlaneConfig {
    pub context: String,
    pub home: String,
    pub can_nuke: bool,
    pub can_scorch: bool,
    pub control_port: u16,
}
/*
pub mod traits;
pub mod factory;
pub mod runner;

 */
use serde::de::{MapAccess, Visitor};
use std::fmt::{Debug, Display};
use crate::hyperspace::platform::PlatformConfig;
use derive_builder::Builder;
use futures::TryFutureExt;
use itertools::Itertools;
use serde::{Deserialize, Deserializer, Serialize};
use serde_yaml::Value;
use std::future::Future;
use std::hash::Hash;
use std::ops::{Deref, DerefMut};
use std::str::FromStr;
use serde;


#[derive(Builder, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
struct DependencyConfigProto{
    pub dependency: Value,
    pub config: Value
}



impl DependencyConfigProto {


}

#[derive(Builder, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
struct ProviderConfigProto {
    pub provider: Value,
    pub config: Value
}






pub struct RegistryConfig2 {

}


#[derive(Clone)]
pub struct LiveService<S> where S: Clone{
    pub service: S,
    tx: tokio::sync::mpsc::Sender<()>
}

impl <S> Deref for LiveService<S> where S: Clone{
    type Target = S;

    fn deref(&self) -> &Self::Target {
        &self.service
    }
}

impl <S> DerefMut for LiveService<S> where S: Clone{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.service
    }
}


#[cfg(test)]
mod test {
    use serde_yaml;
    use crate::hyperspace::foundation::kind::{FoundationKind, PostgresKind, ProviderKind};

    #[test]
    fn test_kind() {
        assert_eq!("foundation: DockerDesktop",format!("{}",serde_yaml::to_string( &FoundationKind::DockerDesktop).unwrap().trim()));
    }


    #[test]
    fn test_provider_kind() {
        let k = ProviderKind::Postgres(PostgresKind::Database);
        println!("{}", k);
    }


}






