use derive_name::Name;
use std::fmt::{Debug, Display};
use std::hash::Hash;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use nom::sequence::tuple;
use nom::bytes::complete::tag;
use serde_with_macros::DeserializeFromStr;
use crate::base::err::BaseErr;
use crate::base::foundation::kind::FoundationKind;
use crate::space::parse::{camel_case, CamelCase};
use crate::space::parse::util::{new_span, result};

pub const FOUNDATION: &'static str = "config";
pub const DEPENDENCY: &'static str = "core";
pub const PROVIDER: &'static str = "provider";

#[derive(
    Name, Clone, Debug, Eq, PartialEq, Hash, strum_macros::Display, Serialize, Deserialize,
)]
#[serde(untagged)]
pub enum Kind {
    #[serde(alias = "{0}")]
    #[strum(to_string = "{0}")]
    Dependency(DependencyKind),
    #[serde(alias = "{0}")]
    #[strum(to_string = "{0}")]
    Provider(ProviderKind),
}

impl IKind for Kind {
    fn category(&self) -> &'static str {
        match self {
            Kind::Dependency(k) => k.category(),
            Kind::Provider(k) => k.category(),
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            Kind::Dependency(kind) => kind.as_str(),
            Kind::Provider(kind) => kind.as_str(),
        }
    }
}

impl IKind for FoundationKind {
    fn category(&self) -> &'static str {
        FOUNDATION
    }

    fn as_str(&self) -> &'static str {
        self.into()
    }
}

#[derive(
    Name,
    Clone,
    Debug,
    Eq,
    PartialEq,
    Hash,
    strum_macros::Display,
    strum_macros::EnumString,
    strum_macros::IntoStaticStr,
    strum_macros::EnumIter,
    Serialize,
    Deserialize,
)]
pub enum DependencyKind {
    PostgresCluster,
    DockerDaemon,
}

impl Into<Kind> for DependencyKind {
    fn into(self) -> Kind {
        Kind::Dependency(self)
    }
}

impl DependencyKind {
    fn provider(&self, provider: CamelCase) -> ProviderKind {
        ProviderKind::new(self.clone(), provider)
    }
}

impl IKind for DependencyKind {
    fn category(&self) -> &'static str {
        DEPENDENCY
    }

    fn as_str(&self) -> &'static str {
        self.into()
    }
}

impl FromStr for ProviderKind {
    type Err=BaseErr;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let i = new_span(s);
        let (dep, _, provider) = result(tuple((camel_case, tag("::"), camel_case))(i))?;

        let dep = DependencyKind::from_str(dep.as_str()).map_err(BaseErr::config_err)?;

        let key = Self { dep, provider };

        Ok(key)
    }
}

impl ProviderKind {
    pub fn new(dep: DependencyKind, provider: CamelCase) -> Self {
        Self { dep, provider }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, DeserializeFromStr, Name)]
pub struct ProviderKind {
    pub dep: DependencyKind,
    pub provider: CamelCase,
}

impl Into<Kind> for ProviderKind {
    fn into(self) -> Kind {
        Kind::Provider(self)
    }
}

impl Display for ProviderKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            format!("{}::{}", self.dep.as_str(), self.provider.as_str()).to_string()
        )
    }
}

#[derive(
    Name,
    Clone,
    Debug,
    Eq,
    PartialEq,
    Hash,
    strum_macros::Display,
    strum_macros::EnumString,
    strum_macros::IntoStaticStr,
    Serialize,
    Deserialize,
)]
#[serde(untagged)]
pub enum PostgresKind {
    Registry,
    Database,
}

impl Default for PostgresKind {
    fn default() -> Self {
        Self::Registry
    }
}

impl IKind for ProviderKind {
    fn category(&self) -> &'static str {
        PROVIDER
    }

    fn as_str(&self) -> &'static str {
        self.into()
    }
}

impl From<&ProviderKind> for &str {
    fn from(kind: &ProviderKind) -> Self {
        kind.as_str()
    }
}

pub trait IKind
where
    for<'z> Self:
        Name + Debug + Clone + Eq + PartialEq + Display + Hash + Serialize + Deserialize<'z>,
{
    fn category(&self) -> &'static str;

    fn as_str(&self) -> &'static str;
}

