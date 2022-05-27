use std::convert::TryInto;
use std::sync::Arc;
use mesh_portal::version::latest::bin::Bin;
use mesh_portal::version::latest::config::bind::BindConfig;
use mesh_portal::version::latest::config::Config;
use mesh_portal_versions::version::v0_0_1::parse::{bind_config, doc};
use crate::artifact::ArtifactRef;
use crate::cache::{CachedConfig};
use crate::error::Error;
use crate::particle::config::Parser;

pub struct BindConfigParser;

impl BindConfigParser {
    pub fn new() -> Self {
        Self {}
    }
}

impl Parser<CachedConfig<BindConfig>> for BindConfigParser {
    fn parse(&self, artifact: ArtifactRef, _data: Bin ) -> Result<Arc<CachedConfig<BindConfig>>, Error> {
        let raw = String::from_utf8(_data.to_vec() )?;
println!("\n{}\n",raw);
        let bind = bind_config(raw.as_str())?;
        let config = CachedConfig {
            artifact_ref : artifact,
            item: bind,
            references: vec![]
        };
        Ok(Arc::new(config))
    }
}
