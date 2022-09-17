use crate::artifact::ArtifactRef;
use crate::cache::{ArtifactItem, Cacheable};
use crate::error::Error;
use cosmic_universe::id2::id::Kind;
use cosmic_universe::id2::ArtifactSubKind;
use mesh_portal::version::latest::command::common::{PropertyMod, SetProperties};
use mesh_portal::version::latest::id::Point;
use mesh_portal::version::latest::particle::Property;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ops::Deref;
use std::str::FromStr;

#[derive(Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct ParticleConfig {
    pub artifact_ref: ArtifactRef,
    pub kind: Kind,
    pub properties: SetProperties,
    pub install: Vec<String>,
}

impl Cacheable for ParticleConfig {
    fn artifact(&self) -> ArtifactRef {
        self.artifact_ref.clone()
    }

    fn references(&self) -> Vec<ArtifactRef> {
        let mut refs = vec![];

        if let Some(property) = self.properties.get(&"bind".to_string()) {
            if let PropertyMod::Set { key, value, lock } = property {
                if let Ok(address) = Point::from_str(value.as_str()) {
                    refs.push(ArtifactRef {
                        point: address,
                        kind: ArtifactSubKind::Bind,
                    })
                }
            }
        }

        refs
    }
}

pub struct ContextualConfig {
    pub config: ArtifactItem<ParticleConfig>,
    pub point: Point,
}

impl ContextualConfig {
    pub fn new(config: ArtifactItem<ParticleConfig>, address: Point) -> Self {
        Self {
            config,
            point: address,
        }
    }

    pub fn substitution_map(&self) -> Result<HashMap<String, String>, Error> {
        let mut map = HashMap::new();
        map.insert("self".to_string(), self.point.to_string());
        map.insert(
            "self.config.bundle".to_string(),
            self.config
                .artifact_ref
                .point
                .clone()
                .to_bundle()?
                .to_string(),
        );
        Ok(map)
    }

    pub fn properties(&self) -> Result<SetProperties, Error> {
        Ok(self.properties.clone())
    }

    pub fn get_property(&self, key: &str) -> Result<String, Error> {
        if let PropertyMod::Set { key, value, lock } =
            self.config.properties.get(&key.to_string()).ok_or(format!(
                "property '{}' required for {} config",
                key,
                self.config.kind.to_string()
            ))?
        {
            Ok(value.to_string())
        } else {
            Err(format!(
                "property '{}' required for {} config",
                key,
                self.config.kind.to_string()
            )
            .into())
        }
    }

    pub fn bind(&self) -> Result<Point, Error> {
        Ok(Point::from_str(self.get_property("bind")?.as_str())?)
    }

    pub fn install(&self) -> Result<Vec<String>, Error> {
        let map = self.substitution_map()?;
        let mut rtn = vec![];
        for line in &self.install {
            rtn.push(line.to_string());
        }
        Ok(rtn)
    }
}

impl Deref for ContextualConfig {
    type Target = ArtifactItem<ParticleConfig>;

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
    pub config: ContextualConfig,
}

impl Deref for MechtronConfig {
    type Target = ContextualConfig;

    fn deref(&self) -> &Self::Target {
        &self.config
    }
}

impl MechtronConfig {
    pub fn from_contextual_config(config: ContextualConfig) -> Self {
        Self { config }
    }

    pub fn new(config: ArtifactItem<ParticleConfig>, address: Point) -> Self {
        let config = ContextualConfig::new(config, address);
        Self { config }
    }

    pub fn wasm_src(&self) -> Result<Point, Error> {
        Ok(Point::from_str(self.get_property("wasm.src")?.as_str())?)
    }

    pub fn mechtron_name(&self) -> Result<String, Error> {
        self.get_property("mechtron.name")
    }

    pub fn validate(&self) -> Result<(), Error> {
        self.wasm_src()?;
        self.mechtron_name()?;
        Ok(())
    }
}
