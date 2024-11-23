use std::fmt::Debug;
use std::marker::PhantomData;
use serde::{de, Deserialize, Deserializer, Serialize};
use serde_yaml::Value;
use crate::hyperspace::foundation::err::FoundationErr;
use crate::hyperspace::foundation::kind::{FoundationKind, IKind};
use crate::hyperspace::foundation::traits::Foundation;



#[derive(Debug, Clone,Serialize,Deserialize,Eq,PartialEq)]
pub struct ProtoFoundationSettings {
   foundation: FoundationKind,
   settings: Value,
}

impl ProtoFoundationSettings {
   pub fn create(self) -> Result<impl Foundation,FoundationErr> {
      self.foundation.parse(self.settings)
   }
}

#[derive(Debug, Clone, Serialize,Eq,PartialEq)]
pub struct FoundationSettings<S> where S: Serialize+Eq+PartialEq {
   foundation: FoundationKind,
   settings: S
}

/*
fn deserialize_from_value<'de, D>(deserializer: D) -> Result<Value, D::Error>
where
    D: Deserializer<'de>,
{
   let value = Deserialize::deserialize(deserializer)?;
   serde_yaml::from_value(value).map_err(de::Error::custom)
}

 */




impl <C> FoundationSettings<C> where C: Serialize+Eq+PartialEq {
   pub fn new(foundation: FoundationKind, settings: C) -> Self {
      Self {
         foundation,
         settings
      }
   }
}

pub type RawSettings = serde_yaml::Value;





#[cfg(test)]
pub mod test {
   use serde_yaml::Value;
   use crate::hyperspace::foundation::DockerDesktopFoundationSettings;
   use crate::hyperspace::foundation::settings::{FoundationSettings, ProtoFoundationSettings};
   use crate::hyperspace::foundation::kind::{DockerDesktopSettings, FoundationKind};

   #[test]
   pub fn test() {
      let settings = DockerDesktopFoundationSettings::new("zophis".to_string());

      let original = FoundationSettings {
         foundation: FoundationKind::DockerDesktop,
         settings
      };


      let data = format!("{}", serde_yaml::to_string(&original).unwrap());

      let settings: ProtoFoundationSettings = serde_yaml::from_str(&data).unwrap();

      let sub_settings: DockerDesktopFoundationSettings = serde_yaml::from_value(settings.settings).unwrap();

      let settings = FoundationSettings::new( settings.foundation, sub_settings);

      assert_eq!(original,settings);

   }
}