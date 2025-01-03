use starlane_space::parse::VarCase;
/// [Partial] definitions for mounting persisted storage

use crate::partial::Partial;
use crate::partial::config::PartialConfig;


pub trait MountsConfig: PartialConfig {
    type VolumeConfig: VolumeConfig;
    fn volumes(&self) -> Vec<Self::VolumeConfig>;
}

pub trait VolumeConfig: PartialConfig {
    /// name must be unique amongst this volumes container: [Mounts]
    fn name(&self) -> VarCase;

    fn path(&self) -> String;
}

#[async_trait::async_trait]
pub trait Mounts: Partial<Config: MountsConfig + Clone> {
    type Volume:Volume;
    fn volumes(&self) -> Vec<Self::Volume>;
}

#[async_trait::async_trait]
pub trait Volume: Partial<Config: VolumeConfig + Clone> {}


