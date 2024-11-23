use std::fmt::Debug;
use std::marker::PhantomData;
use serde::{Deserialize, Deserializer, Serialize};
use serde_yaml::Value;
use crate::hyperspace::foundation::err::FoundationErr;
use crate::hyperspace::foundation::kind::{FoundationKind, IKind};
use crate::hyperspace::foundation::traits::Foundation;

#[derive(Debug, Clone,Serialize,Deserialize)]
pub struct ProtoFoundationConfig {
   foundation: FoundationKind,
   #[serde(flatten)]
   config: Value,
}

impl ProtoFoundationConfig {
   pub fn create(self) -> Result<impl Foundation,FoundationErr> {
      self.foundation.parse(self.config)
   }
}



#[derive(Debug, Clone,Serialize)]
pub struct FoundationConfig<C> where C: Serialize{
   foundation: FoundationKind,
   config: C
}

impl <C> FoundationConfig<C> where C: Serialize{
   pub fn new(foundation: FoundationKind, config: C) -> Self {
      Self {
         foundation,
         config
      }
   }
}

pub type RawConfig = serde_yaml::Value;


/*
impl <'a,K,C> Deserialize<'a> for Config<K,C> where K: IKind+Deserialize<'a>+'a, C: Debug+Clone+Deserialize<'a>+'a{
   fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
   where
       D: Deserializer<'a>
   {
      todo!()
   }
}

 */



#[cfg(test)]
pub mod test {
   use serde_yaml::Value;
   use crate::hyperspace::foundation::config::{ FoundationConfig, ProtoFoundationConfig};
   use crate::hyperspace::foundation::kind::{DockerDesktopConfig, FoundationKind};

   #[test]
   pub fn test() {
      let config = DockerDesktopConfig::new("zophis".to_string());

      let config = FoundationConfig {
         foundation: FoundationKind::DockerDesktop,
         config
      };


      let data = format!("{}", serde_yaml::to_string(&config).unwrap());

      let config: ProtoFoundationConfig = serde_yaml::from_str(&data).unwrap();


      /*
      let conf_str = r#"
foundation: DockerDesktop
config:
  go: true
"#;

      let conf: ProtoConfig = serde_yaml::from_str(conf_str).unwrap();

       */

   }
}