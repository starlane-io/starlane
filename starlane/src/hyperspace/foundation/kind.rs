use serde::{Deserialize, Deserializer, Serialize};
use std::fmt::{Debug, Display};
use std::hash::Hash;
use std::str::FromStr;
use derive_name::Name;
use thiserror::__private::AsDisplay;
use crate::hyperspace::foundation::err::FoundationErr;
use crate::hyperspace::foundation::kind::ProviderKind::DockerDaemon;
use crate::hyperspace::foundation::traits::{Dependency, Foundation};

pub const FOUNDATION : &'static str = "foundation";
pub const DEPENDENCY: &'static str = "implementation";
pub const PROVIDER: &'static str = "provider";


#[derive(Name,Clone,Debug,Eq,PartialEq,Hash,strum_macros::Display,strum_macros::EnumString, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Kind {
   #[serde(alias="{0}")]
   #[strum(to_string="{0}")]
   Foundation(FoundationKind),
   #[serde(alias="{0}")]
   #[strum(to_string="{0}")]
   Dependency(DependencyKind),
   #[serde(alias="{0}")]
   #[strum(to_string="{0}")]
   Provider(ProviderKind)
}

impl IKind for Kind {
    fn category(&self) -> &'static str {
        match self {
            Kind::Foundation(k) => k.category(),
            Kind::Dependency(k) => k.category(),
            Kind::Provider(k) => k.category()
        }
    }

    fn as_str(&self) -> &'static str {
       match self {
           Kind::Foundation(kind) => kind.as_str(),
           Kind::Dependency(kind) => kind.as_str(),
           Kind::Provider(kind) => kind.as_str()
       }
    }
}


#[derive(Name,Clone,Debug,Eq,PartialEq,Hash,strum_macros::Display,strum_macros::EnumString,strum_macros::EnumIter, strum_macros::IntoStaticStr, Serialize, Deserialize)]
pub enum FoundationKind {
    DockerDesktop
}

impl Default for FoundationKind {
    fn default() -> Self {
      Self::DockerDesktop
    }
}

//pub type FoundationParser = fn(&Value) -> Result<dyn Foundation, FoundationErr>;





impl IKind for FoundationKind {

    fn category(&self) -> &'static str {
        FOUNDATION
    }

    fn as_str(&self) -> &'static str  {
        self.into()
    }

}

#[derive(Name,Clone,Debug,Eq,PartialEq,Hash,strum_macros::Display,strum_macros::EnumString,strum_macros::IntoStaticStr,strum_macros::EnumIter, Serialize, Deserialize)]
#[serde(tag="implementation")]
pub enum DependencyKind {
    Postgres,
    Docker
}

impl Default for DependencyKind {
    fn default() -> Self {
        todo!()
    }
}

impl IKind for DependencyKind{
    fn category(&self) -> &'static str {
        DEPENDENCY
    }

    fn as_str(&self) -> &'static str  {
        self.into()
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

#[derive(Name,Clone,Debug,Eq,PartialEq,Hash,strum_macros::Display, strum_macros::IntoStaticStr, strum_macros::EnumString, Serialize, Deserialize)]
#[serde(tag="provider")]
pub enum ProviderKind {
    #[serde(alias="Postgres::{0}")]
    #[strum(to_string = "Postgres::{0}")]
    Postgres(PostgresKind),
    DockerDaemon
}

impl Default for ProviderKind {
    fn default() -> Self {
        Self::Postgres(Default::default())
    }
}

#[derive(Name,Clone,Debug,Eq,PartialEq,Hash,strum_macros::Display,strum_macros::EnumString, strum_macros::IntoStaticStr,Serialize, Deserialize)]
#[serde(untagged)]
pub enum PostgresKind{
    Registry,
    Database
}

impl Default for PostgresKind  {
    fn default() -> Self {
        Self::Registry
    }
}

impl IKind for ProviderKind{

    fn category(&self) -> &'static str {
        PROVIDER
    }

    fn as_str(&self) -> &'static str {
        self.into()
    }
}

pub trait IKind where for<'a> Self: FromStr+Name+Debug+Clone+Eq+PartialEq+Display+Hash+Serialize+Deserialize<'a> {
  fn category(&self) -> &'static str;

  fn as_str(&self) -> &'static str;
}




#[cfg(test)]
pub mod test {
    use serde::Serialize;
    use crate::hyperspace::foundation::kind::{DependencyKind, IKind, Kind};

    #[test]
    pub fn test( )  {

        fn ser<K>( kind: &K ) where K: IKind+ToString+Serialize{
            let id = kind.category();
            let kind_str = serde_yaml::to_string(kind).unwrap().trim().to_string();
println!("{}",kind_str);
            assert_eq!(kind_str.as_str(),format!("{}: {}", id, kind.to_string()))
        }

        //ser(&Kind::Foundation(FoundationKind::DockerDesktop));
        ser(&Kind::Dependency(DependencyKind::Postgres));
        //ser(&Kind::Provider(ProviderKind::Postgres(PostgresKind::Database)));

        /*
        assert_eq!("{}",serde_yaml::to_string(&kind).unwrap());

        let kind = FoundationKind::DockerDesktop;
        println!("{}",serde_yaml::to_string(&kind).unwrap());

        let string = serde_yaml::to_string(&kind).unwrap();
        let kind : FoundationKind = serde_yaml::from_str(string.as_str()).unwrap();
        println!("{}", kind);

        let kind = Kind::Foundation(FoundationKind::DockerDesktop);
        let string = serde_yaml::to_string(&kind).unwrap();
        println!("input: '{}'",string.trim());
        let kind : Kind  = serde_yaml::from_str(string.as_str()).unwrap();

        println!("result: '{}'",kind.to_string());
        assert_eq!("DockerDesktop",kind.to_string().as_str());

         */
    }

}
#[derive(Debug, Clone,Serialize, Deserialize,Eq,PartialEq)]
pub struct DockerDesktopSettings {
    pub name: String
}

impl DockerDesktopSettings {
    pub fn new( name: String ) -> Self {
        Self {
            name
        }
    }
}

impl FromStr for DockerDesktopSettings {
    type Err = FoundationErr;

    fn from_str(settings: &str) -> Result<Self, Self::Err> {
        serde_yaml::from_str(settings).map_err(|err|FoundationErr::foundation_verbose_error(FoundationKind::DockerDesktop, err, settings))
    }
}
