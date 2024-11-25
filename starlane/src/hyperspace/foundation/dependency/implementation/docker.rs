use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use crate::hyperspace::foundation::config;
use crate::hyperspace::foundation::err::FoundationErr;
use crate::hyperspace::foundation::kind::ProviderKind;
use crate::hyperspace::foundation::util::Map;
use crate::space::parse::CamelCase;

#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct DockerDaemonConfig {
    pub providers: HashMap<CamelCase,DockerProviderConfig>,
}


impl DockerDaemonConfig {
    pub fn create(config: Map) -> Result<impl config::DependencyConfig, FoundationErr> {

        let providers = config.parse_same("providers" )?;

        Ok(DockerDaemonConfig {
            providers,
        })
    }

}


#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct DockerProviderConfig  {
    kind: CamelCase
}
