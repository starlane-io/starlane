//pub mod docker;
pub mod settings;
pub mod err;

pub mod kind;

pub mod traits;



#[derive(Clone, Serialize, Deserialize)]
pub struct StarlaneSettings {
    pub context: String,
    pub home: String,
    pub can_nuke: bool,
    pub can_scorch: bool,
    pub control_port: u16,
    pub foundation: ProtoFoundationSettings
}

impl StarlaneSettings {
    pub fn create_foundation(&self) -> Result<impl Foundation,FoundationErr> {
        self.foundation.clone().create()
    }
}

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
use std::collections::HashSet;
use serde::de::{MapAccess, Visitor};
use std::fmt::{Debug, Display};
use crate::hyperspace::platform::PlatformConfig;
use derive_builder::Builder;
use futures::TryFutureExt;
use itertools::Itertools;
use serde::{de, Deserialize, Deserializer, Serialize};
use serde_yaml::Value;
use std::future::Future;
use std::hash::Hash;
use std::ops::{Deref, DerefMut};
use std::str::FromStr;
use serde;
use crate::hyperspace::foundation::settings::{FoundationSettings, ProtoFoundationSettings, RawSettings};
use crate::hyperspace::foundation::err::FoundationErr;
use crate::hyperspace::foundation::kind::{DependencyKind, FoundationKind, IKind};
use crate::hyperspace::foundation::traits::{Dependency, Foundation};

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





#[derive(Debug, Clone,Serialize,Deserialize,Eq,PartialEq)]
pub struct DockerDesktopFoundationSettings {
    name: String
}

impl DockerDesktopFoundationSettings {
   pub fn new(name: String) -> Self {
       Self {
           name
       }
   }
}



pub struct DockerDesktopFoundation {
    settings: FoundationSettings<DockerDesktopFoundationSettings>
}

impl DockerDesktopFoundation {
    pub fn new(config: DockerDesktopFoundationSettings) -> Self {
        let config = FoundationSettings::new(FoundationKind::DockerDesktop, config);
        Self {
            settings: config
        }
    }

}

impl Foundation for DockerDesktopFoundation {
    fn kind(&self) -> FoundationKind {
        Self::foundation_kind()
    }

    fn foundation_kind() -> FoundationKind {
        FoundationKind::DockerDesktop
    }

    fn parse(config: RawSettings) -> Result<impl Foundation+Sized, FoundationErr>{
        Ok(Self::new(serde_yaml::from_value(config.clone()).map_err(|err| FoundationErr::foundation_conf_err(Self::foundation_kind(), err, config.clone()))?))
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


#[cfg(test)]
pub mod test {
    use derive_name::Named;
    use crate::hyperspace::foundation::settings::ProtoFoundationSettings;
    use crate::hyperspace::foundation::err::FoundationErr;

    #[test]
    pub fn test() -> Result<(),FoundationErr>{
       let settings = r#"
foundation: DockerDesktop
name: Hello
settings:
  name: "Filo Farnsworth"

       "#;

       let settings: ProtoFoundationSettings = serde_yaml::from_str(settings).unwrap();
        println!("{:?}", settings);

        let ser = serde_yaml::to_string(&settings).unwrap();
        println!("{}", ser );
       let foundation = settings.create()?;


        /*
        println!("{} dependencies found in {}", foundation.dependencies().len(), foundation.kind() );
        for dep in foundation.dependencies() {
            println!("{}", dep );
        }


         */
        Ok(())

    }
}
