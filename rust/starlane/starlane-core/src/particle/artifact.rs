use mesh_portal::version::latest::command::common::StateSrc;
use mesh_portal::version::latest::id::Point;
use std::convert::{TryFrom, TryInto};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::error::Error;

#[derive(Clone)]
pub struct ArtifactBundle {
    address: Point,
    state_src: StateSrc,
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
