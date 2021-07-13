use crate::resource::ArtifactBundleKey;
use crate::error::Error;
use crate::resource::{ResourceAddress, SrcTransfer};
use serde::{Deserialize, Serialize};
use std::convert::{TryFrom, TryInto};
use std::sync::Arc;

#[derive(Clone)]
pub struct ArtifactBundle {
    key: ArtifactBundleKey,
    address: ResourceAddress,
    state_src: SrcTransfer<ArtifactBundleState>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ArtifactBundleState {
    content: Arc<Vec<u8>>,
}

impl ArtifactBundleState {
    pub fn new(content: Arc<Vec<u8>>) -> Self {
        ArtifactBundleState { content: content }
    }
}

impl TryInto<Vec<u8>> for ArtifactBundleState {
    type Error = Error;

    fn try_into(self) -> Result<Vec<u8>, Self::Error> {
        Ok(bincode::serialize(&self)?)
    }
}

impl TryInto<Arc<Vec<u8>>> for ArtifactBundleState {
    type Error = Error;

    fn try_into(self) -> Result<Arc<Vec<u8>>, Self::Error> {
        Ok(Arc::new(bincode::serialize(&self)?))
    }
}

impl TryFrom<Arc<Vec<u8>>> for ArtifactBundleState {
    type Error = Error;

    fn try_from(value: Arc<Vec<u8>>) -> Result<Self, Self::Error> {
        Ok(bincode::deserialize::<ArtifactBundleState>(
            value.as_slice(),
        )?)
    }
}

impl TryFrom<Vec<u8>> for ArtifactBundleState {
    type Error = Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        Ok(bincode::deserialize::<ArtifactBundleState>(
            value.as_slice(),
        )?)
    }
}
