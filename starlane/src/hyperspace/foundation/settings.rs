use std::fmt::Debug;
use std::marker::PhantomData;
use std::str::FromStr;
use serde::{de, Deserialize, Deserializer, Serialize};
use serde_yaml::Value;
use crate::hyperspace::foundation::err::FoundationErr;
use crate::hyperspace::foundation::kind::{DockerDesktopSettings, FoundationKind, IKind};
use crate::hyperspace::foundation::traits::Foundation;

/// Settings are provided by the User.


#[derive(Debug, Clone,Serialize,Deserialize,Eq,PartialEq)]
pub struct ProtoFoundationSettings {
   foundation: FoundationKind,
   settings: Value,
}

impl ProtoFoundationSettings {
   pub fn new(foundation: FoundationKind, settings: Value ) -> Self {
      Self { foundation, settings }
   }
   pub fn create<S>(self) -> Result<FoundationSettings<S>,FoundationErr> where S: Eq+PartialEq+for<'z> Deserialize<'z>{
      serde_yaml::from_value(self.settings.clone()).map_err(FoundationErr::settings_err)
   }

}

use std::fmt::Display;

#[derive(Debug, Clone,Eq,PartialEq,Serialize,Deserialize)]
pub struct FoundationSettings<S> where S: Eq+PartialEq,  {
   foundation: FoundationKind,
   #[serde(bound(deserialize = "S: Deserialize<'de>"))]
   settings: S
}


fn deserialize_from_value<D, S>(deserializer: D) -> Result<S, <D as Deserializer<'static>>::Error>
where
    for<'de> <D as Deserializer<'de>>::Error: Display,
    for<'de> D:  Deserializer<'de,Error = serde_yaml::Error>,
    for<'de> S: Deserialize<'de>+Eq+PartialEq
{
   let value = Deserialize::deserialize(deserializer)?;
   serde_yaml::from_value(value)
}


impl <C> FoundationSettings<C> where C: for<'z> Deserialize<'z>+Eq+PartialEq {
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
   use crate::hyperspace::foundation::implementation::docker_desktop_foundation::DockerDesktopFoundationSettings;
   use crate::hyperspace::foundation::settings::{FoundationSettings, ProtoFoundationSettings};
   use crate::hyperspace::foundation::kind::{DockerDesktopSettings, FoundationKind};

   #[test]
   pub fn test() {
      let settings = DockerDesktopFoundationSettings::new("zophis".to_string());

      let original = FoundationSettings {
         foundation: FoundationKind::DockerDaemon,
         settings
      };


      let data = format!("{}", serde_yaml::to_string(&original).unwrap());

      let settings: ProtoFoundationSettings = serde_yaml::from_str(&data).unwrap();

      let sub_settings: DockerDesktopFoundationSettings = serde_yaml::from_value(settings.settings).unwrap();

      let settings = FoundationSettings::new( settings.foundation, sub_settings);

      assert_eq!(original,settings);

   }
}