//pub mod docker;
pub mod config;
pub mod err;

pub mod kind;

pub mod traits;

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
use std::collections::HashSet;
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
use crate::hyperspace::foundation::config::{FoundationConfig, ProtoFoundationConfig, RawConfig};
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





#[derive(Debug, Clone,Serialize,Deserialize)]
pub struct DockerDesktopFoundationConfig {
    name: String
}



pub struct DockerDesktopFoundation {
    config: FoundationConfig<DockerDesktopFoundationConfig>
}

impl DockerDesktopFoundation {
    pub fn new(config: DockerDesktopFoundationConfig) -> Self {
        let config = FoundationConfig::new(FoundationKind::DockerDesktop, config);
        Self {
            config
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

    fn parse(config: RawConfig) -> Result<impl Foundation+Sized, FoundationErr>{
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
    use crate::hyperspace::foundation;
    use crate::hyperspace::foundation::config::ProtoFoundationConfig;
    use crate::hyperspace::foundation::DockerDesktopFoundationConfig;
    use crate::hyperspace::foundation::err::FoundationErr;
    use crate::hyperspace::foundation::traits::Foundation;

    #[test]
    pub fn test() -> Result<(),FoundationErr>{
       let config = r#"
foundation: DockerDesktop
config:
  name: "Filo Farnsworth"

       "#;

       let config: ProtoFoundationConfig = serde_yaml::from_str(config).unwrap();
        println!("{:?}", config );
        let ser = serde_yaml::to_string(&config).unwrap();
        println!("{}", ser );
       let foundation = config.create()?;


        /*
        println!("{} dependencies found in {}", foundation.dependencies().len(), foundation.kind() );
        for dep in foundation.dependencies() {
            println!("{}", dep );
        }


         */
        Ok(())

    }
}
