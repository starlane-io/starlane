use crate::cache::{ArtifactItem, ArtifactCaches};
use crate::config::mechtron::MechtronConfig;
use crate::config::wasm::Wasm;
use crate::config::bind::BindConfig;
use crate::error::Error;

#[derive(Clone)]
pub struct Mechtron {
    pub config: ArtifactItem<MechtronConfig>,
    pub wasm: ArtifactItem<Wasm>,
    pub bind_config: ArtifactItem<BindConfig>
}

impl Mechtron {
    pub fn new( config: ArtifactItem<MechtronConfig>, caches: &ArtifactCaches ) -> Result<Self,Error> {

        let wasm = caches.wasms.get(&config.wasm.address ).ok_or(format!("could not get referenced Wasm: {}", config.wasm.address.to_string()) )?;
        let bind_config = caches.bind_configs.get(&config.bind.address ).ok_or::<Error>(format!("could not get referenced BindConfig: {}", config.wasm.address.to_string()).into() )?;

        Ok(Self{
            config,
            wasm,
            bind_config
        })
    }
}