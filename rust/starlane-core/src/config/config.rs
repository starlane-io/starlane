use std::collections::HashMap;
use std::ops::Deref;
use std::str::FromStr;
use mesh_portal_serde::version::latest::command::common::{PropertyMod, SetProperties};
use mesh_portal_serde::version::latest::id::Address;
use mesh_portal_serde::version::latest::resource::Property;
use crate::artifact::ArtifactRef;
use crate::cache::{ArtifactItem, Cacheable};
use crate::command::compose::{Command, CommandOp};
use crate::config::parse::replace::substitute;
use crate::error::Error;
use crate::resource::{ArtifactKind, Kind};

pub struct ResourceConfig {
    pub artifact_ref: ArtifactRef,
    pub kind: Kind,
    pub properties: SetProperties,
    pub install: Vec<String>
}


impl Cacheable for ResourceConfig {
    fn artifact(&self) -> ArtifactRef {
        self.artifact_ref.clone()
    }

    fn references(&self) -> Vec<ArtifactRef> {

        let mut refs = vec![];

        if let Some(property) = self.properties.get(&"bind".to_string() ) {
            if let PropertyMod::Set{ key, value,lock } = property {
                if let Ok(address) = Address::from_str(value.as_str()) {
                    refs.push( ArtifactRef {
                        address,
                        kind: ArtifactKind::Bind
                    })
                }
            }
        }

        if let Some(property) = self.properties.get(&"wasm.src".to_string() ) {
            if let PropertyMod::Set{ key, value,lock } = property {
                if let Ok(address) = Address::from_str(value.as_str()) {
                    refs.push( ArtifactRef {
                        address,
                        kind: ArtifactKind::Wasm
                    })
                }
            }
        }

        refs
    }
}

pub struct ContextualConfig {
  pub config: ArtifactItem<ResourceConfig>,
  pub address: Address
}

impl ContextualConfig {
    pub fn new( config: ArtifactItem<ResourceConfig>, address: Address ) -> Self {
        Self {
            config,
            address
        }
    }

    pub fn substitution_map(&self) -> Result<HashMap<String,String>,Error> {
      let mut map = HashMap::new();
      map.insert( "self".to_string(), self.address.to_string() );
      map.insert( "self.config.bundle".to_string(), self.config.artifact_ref.address.clone().to_bundle()?.to_string() );
      Ok(map)
  }

  pub fn properties( &self ) -> Result<SetProperties,Error> {
     let map = self.substitution_map()?;
     let mut rtn = SetProperties::new();
     for (_,property) in  &self.config.properties.map {
          if let PropertyMod::Set { key, value, lock } = property {
              let value = substitute(value.as_str(), &map )?;
              let property = PropertyMod::Set {
                  key: key.to_string(),
                  value,
                  lock: lock.clone()
              };
              rtn.push(property);
          }
     }
     Ok(rtn)
  }

  pub fn get_property( &self, key: &str ) -> Result<String,Error> {
      if let PropertyMod::Set{ key, value, lock } = self.config.properties.get(&key.to_string() ).ok_or(format!("property '{}' required for {} config", key, self.config.kind.to_string() ))? {
          Ok(substitute(value.as_str(), &self.substitution_map()?)?.to_string())
      } else {
          Err(format!("property '{}' required for {} config", key, self.config.kind.to_string()).into() )
      }
  }

  pub fn bind(&self) -> Result<Address,Error> {
        Ok(Address::from_str(self.get_property("bind")?.as_str() )?)
  }

  pub fn install(&self) -> Result<Vec<String>,Error> {
      let map = self.substitution_map()?;
      let mut rtn = vec![];
      for line in &self.install {
          rtn.push( substitute(line.as_str(), &map )? );
      }
      Ok(rtn)
  }

}

impl Deref for ContextualConfig {
    type Target =  ArtifactItem<ResourceConfig>;

    fn deref(&self) -> &Self::Target {
        &self.config
    }
}

impl Into<MechtronConfig> for ContextualConfig {
    fn into(self) -> MechtronConfig {
        MechtronConfig::from_contextual_config(self)
    }
}


pub struct MechtronConfig {
    pub config: ContextualConfig
}

impl Deref for MechtronConfig {
    type Target =  ContextualConfig;

    fn deref(&self) -> &Self::Target {
        &self.config
    }
}

impl MechtronConfig {

    pub fn from_contextual_config( config: ContextualConfig ) -> Self {
        Self {
            config
        }
    }

    pub fn new( config: ArtifactItem<ResourceConfig>, address: Address ) -> Self {
        let config = ContextualConfig::new(config,address);
        Self{
            config
        }
    }

    pub fn wasm_src(&self) -> Result<Address,Error> {
        Ok(Address::from_str(self.get_property("wasm.src")?.as_str() )?)
    }

    pub fn mechtron_name(&self) -> Result<String,Error> {
        self.get_property("mechtron.name")
    }

    pub fn validate(&self) -> Result<(),Error> {
        self.wasm_src()?;
        self.mechtron_name()?;
        Ok(())
    }
}

