use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display};
use std::hash::Hash;

#[derive(Clone,Debug,Eq,PartialEq,Hash,strum_macros::Display, Serialize, Deserialize)]
pub enum Kind {
   Foundation(FoundationKind),
   Dependency(DependencyKind),
   Provider(ProviderKind)
}

impl IKind for Kind {
    fn identifier(&self) -> &'static str {
        match self {
            Kind::Foundation(k) => k.identifier(),
            Kind::Dependency(k) => k.identifier(),
            Kind::Provider(k) => k.identifier()
        }
    }
}

#[derive(Clone,Debug,Eq,PartialEq,Hash,strum_macros::Display,strum_macros::EnumString, Serialize, Deserialize)]
#[serde(tag="foundation")]
pub enum FoundationKind {
    DockerDesktop
}

impl IKind for FoundationKind {
    fn identifier(&self) -> &'static str {
        "foundation"
    }
}

#[derive(Clone,Debug,Eq,PartialEq,Hash,strum_macros::Display,strum_macros::EnumString, Serialize, Deserialize)]
#[serde(tag="dependency")]
pub enum DependencyKind {
    Postgres,
    Docker
}

impl IKind for DependencyKind{
    fn identifier(&self) -> &'static str {
        "dependency"
    }
}

#[derive(Clone,Debug,Eq,PartialEq,Hash)]
pub struct ProviderKey{
    dep: DependencyKind,
    kind: ProviderKind
}

impl Display for ProviderKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", format!("{}:{}", self.dep, self.kind))
    }
}

impl ProviderKey {
    pub fn new(dep: DependencyKind, kind: ProviderKind) -> Self {
        Self {
            dep,
            kind,
        }
    }
}

#[derive(Clone,Debug,Eq,PartialEq,Hash,strum_macros::Display, Serialize, Deserialize)]
#[serde(tag="provider")]
pub enum ProviderKind {
    #[strum(to_string = "Postgres<{0}>")]
    Postgres(PostgresKind),
    DockerDaemon
}

#[derive(Clone,Debug,Eq,PartialEq,Hash,strum_macros::Display,strum_macros::EnumString, Serialize, Deserialize)]
pub enum PostgresKind{
    Database
}

impl IKind for ProviderKind{
    fn identifier(&self) -> &'static str {
        "provider"
    }
}

pub trait IKind where Self: Debug+Clone+Eq+PartialEq+Display+Hash {
  fn identifier(&self) -> &'static str;
}