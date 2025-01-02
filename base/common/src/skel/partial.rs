use crate::partial;
use partial::config;

/// this is another skel template example for implementing a [partial::Partial]
pub trait MountsConfig: config::PartialConfig {
    type VolumeConfig: VolumeConfig;
    fn volumes(&self) -> Vec<Self::VolumeConfig>;
}

pub trait VolumeConfig: config::PartialConfig {
    /// name must be unique amongst this volumes container: [partial:Mounts]
    fn name(&self) -> String;
    fn path(&self) -> String;
}

#[async_trait::async_trait]
pub trait Mounts: partial::Partial<Config: MountsConfig + Clone> {
    type Volume: Volume;
    fn volumes(&self) -> Vec<Self::Volume>;
}

#[async_trait::async_trait]
pub trait Volume: partial::Partial<Config: VolumeConfig + Clone> {}



