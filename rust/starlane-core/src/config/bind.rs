use std::convert::TryInto;
use std::sync::Arc;
use mesh_portal::version::latest::bin::Bin;
use mesh_portal::version::latest::config::bind::BindConfig;
use mesh_portal::version::latest::config::Config;
use mesh_portal_versions::version::v0_0_1::config::bind::parse::{bind, final_bind};
use crate::artifact::ArtifactRef;
use crate::cache::{CachedConfig};
use crate::error::Error;
use crate::resource::config::Parser;

pub struct BindConfigParser;

impl BindConfigParser {
    pub fn new() -> Self {
        Self {}
    }
}

impl Parser<CachedConfig<BindConfig>> for BindConfigParser {
    fn parse(&self, artifact: ArtifactRef, _data: Bin ) -> Result<Arc<CachedConfig<BindConfig>>, Error> {
        let raw = String::from_utf8(_data.to_vec() )?;
        let bind = final_bind(raw.as_str())?;
        let bind:BindConfig = bind.try_into()?;
        let config = CachedConfig {
            artifact_ref : artifact,
            item: bind,
            references: vec![]
        };
        Ok(Arc::new(config))
    }
}
