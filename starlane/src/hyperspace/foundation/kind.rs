use serde::{Deserialize, Deserializer, Serialize};
use std::fmt::{Debug, Display};
use std::hash::Hash;
use std::str::FromStr;
use derive_name::Name;
use nom::bytes::complete::tag;
use nom::sequence::tuple;
use serde_with_macros::DeserializeFromStr;
use thiserror::__private::AsDisplay;
use crate::hyperspace::foundation::{Dependency, Foundation};
use crate::hyperspace::foundation::err::FoundationErr;
use crate::space::parse::{camel_case, CamelCase};
use crate::space::parse::util::{new_span, result};

pub const FOUNDATION : &'static str = "config";
pub const DEPENDENCY: &'static str = "implementation";
pub const PROVIDER: &'static str = "provider";


#[derive(Name,Clone,Debug,Eq,PartialEq,Hash,strum_macros::Display,strum_macros::EnumString, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Kind {
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
            Kind::Dependency(k) => k.category(),
            Kind::Provider(k) => k.category()
        }
    }

    fn as_str(&self) -> &'static str {
       match self {
           Kind::Dependency(kind) => kind.as_str(),
           Kind::Provider(kind) => kind.as_str()
       }
    }
}




#[derive(Name,Clone,Debug,Eq,PartialEq,Hash,strum_macros::Display,strum_macros::EnumString,strum_macros::EnumIter, strum_macros::IntoStaticStr, Serialize, Deserialize)]
pub enum FoundationKind {
    DockerDaemon
}

impl Default for FoundationKind {
    fn default() -> Self {
      Self::DockerDaemon
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
pub enum DependencyKind {
    PostgresCluster,
    DockerDaemon
}

impl Into<Kind> for DependencyKind {
    fn into(self) -> Kind {
        Kind::Dependency(self)
    }
}

impl DependencyKind {
    fn provider( &self, provider: CamelCase ) -> ProviderKind {
        ProviderKind::new(self.clone(),provider)
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





impl FromStr for ProviderKind {
    type Err = FoundationErr;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let i = new_span(s);
        let (dep,_,provider) = result(tuple((camel_case,tag("::"),camel_case)))?;

        let dep = DependencyKind::from_str(dep.to_string()).map_err(FoundationErr::config_err)?;

        let key = Self {
            dep,
            provider
        };

        Ok(key)
    }
}

impl ProviderKind {
    pub fn new(dep: DependencyKind, provider: CamelCase) -> Self {
        Self {
            dep,
            provider,
        }
    }
}

#[derive(Clone,Debug,Eq,PartialEq,Hash,Serialize,DeserializeFromStr)]
pub struct ProviderKind {
    pub dep: DependencyKind,
    pub provider: CamelCase
}



impl Into<Kind> for ProviderKind{
    fn into(self) -> Kind {
        Kind::Provider(self)
    }
}

impl Display for ProviderKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", format!("{}::{}", self.dep.as_str(), self.provider.as_str()).to_string())
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
        ser(&Kind::Dependency(DependencyKind::PostgresCluster));
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
        serde_yaml::from_str(settings).map_err(|err|FoundationErr::foundation_verbose_error(FoundationKind::DockerDaemon, err, settings))
    }
}
