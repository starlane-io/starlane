use serde::{Deserialize, Deserializer, Serialize};
use std::fmt::{Debug, Display};
use std::hash::Hash;
use serde_yaml::Value;
use thiserror::__private::AsDisplay;
use crate::hyperspace::foundation::settings::{ProtoFoundationSettings, RawSettings};
use crate::hyperspace::foundation::{DockerDesktopFoundation, DockerDesktopFoundationSettings};
use crate::hyperspace::foundation::config::RawConfig;
use crate::hyperspace::foundation::err::FoundationErr;
use crate::hyperspace::foundation::traits::{Dependency, Foundation};

pub const FOUNDATION : &'static str = "foundation";
pub const DEPENDENCY: &'static str = "dependency";
pub const PROVIDER: &'static str = "provider";


#[derive(Clone,Debug,Eq,PartialEq,Hash,strum_macros::Display, Serialize, Deserialize)]
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


#[derive(Clone,Debug,Eq,PartialEq,Hash,strum_macros::Display,strum_macros::EnumString,strum_macros::EnumIter, strum_macros::IntoStaticStr, Serialize, Deserialize)]
pub enum FoundationKind {
    #[serde(alias="DockerDesktop")]
    DockerDesktop
}

//pub type FoundationConfigParser<'e,C: Deserialize<'e>+'e> = dyn FnMut(Value) -> Result<C,FoundationErr> + Sync + Send+ 'static;

pub type FoundationParser = fn(&Value) -> Result<dyn Foundation, FoundationErr>;

/*
pub fn config_parser(&self) -> fn(&Value) -> Result<Self, FoundationErr> {
    |value| serde_yaml::from_value( value.clone() ).map_err(|err|FoundationErr::foundation_conf_err(FoundationKind::DockerDesktop,err,value.clone()))
}

 */
impl FoundationKind {
   pub fn parse_settings(&self, settings: RawSettings) -> Result<impl Foundation+Sized, FoundationErr> {
       match self {
           FoundationKind::DockerDesktop => DockerDesktopFoundation::parse(settings)
       }
   }

   pub fn parse_config(&self, config: RawConfig ) -> Result<impl Foundation+Sized, FoundationErr> {
        match self {
            FoundationKind::DockerDesktop => DockerDesktopFoundation::parse(config)
        }
    }
}



impl IKind for FoundationKind {

    fn category(&self) -> &'static str {
        FOUNDATION
    }

    fn as_str(&self) -> &'static str  {
        self.into()
    }

}

#[derive(Clone,Debug,Eq,PartialEq,Hash,strum_macros::Display,strum_macros::EnumString,strum_macros::IntoStaticStr,strum_macros::EnumIter, Serialize, Deserialize)]
#[serde(tag="dependency")]
pub enum DependencyKind {
    Postgres,
    Docker
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

#[derive(Clone,Debug,Eq,PartialEq,Hash,strum_macros::Display, strum_macros::IntoStaticStr, Serialize, Deserialize)]
#[serde(tag="provider")]
pub enum ProviderKind {
    #[serde(alias="Postgres::{0}")]
    #[strum(to_string = "Postgres::{0}")]
    Postgres(PostgresKind),
    DockerDaemon
}

#[derive(Clone,Debug,Eq,PartialEq,Hash,strum_macros::Display,strum_macros::EnumString, strum_macros::IntoStaticStr,Serialize, Deserialize)]
#[serde(untagged)]
pub enum PostgresKind{
    Registry,
    Database
}

impl IKind for ProviderKind{

    fn category(&self) -> &'static str {
        PROVIDER
    }

    fn as_str(&self) -> &'static str {
        self.into()
    }
}

pub trait IKind where for<'a> Self: Debug+Clone+Eq+PartialEq+Display+Hash+Serialize+Deserialize<'a> {
  fn category(&self) -> &'static str;

  fn as_str(&self) -> &'static str;
}




#[cfg(test)]
pub mod test {
    use serde::{Deserialize, Serialize};
    use crate::hyperspace::foundation::kind::{DependencyKind, FoundationKind, IKind, Kind, PostgresKind, ProviderKind};

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
