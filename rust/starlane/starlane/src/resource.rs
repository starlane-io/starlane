use cosmic_api::version::v0_0_1::id::StarKey;
use serde::{Serialize,Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLocation {
    pub star: StarKey,
}

impl ResourceLocation {
    pub fn new(host: StarKey) -> Self {
        ResourceLocation { star: host }
    }

    pub fn root() -> Self {
        Self {
            star: StarKey::central(),
        }
    }
}
