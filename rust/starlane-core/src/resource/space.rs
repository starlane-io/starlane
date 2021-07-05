use crate::error::Error;
use crate::keys::{ResourceKey, SpaceKey};
use crate::resource::{AssignResourceStateSrc, LocalDataSrc, Resource, ResourceAddress, ResourceType, SrcTransfer, InitArgs, ResourceKind};
use serde::{Deserialize, Serialize};
use std::convert::{TryFrom, TryInto};
use std::sync::Arc;

#[derive(Clone)]
pub struct Space {
    key: SpaceKey,
    address: ResourceAddress,
    state_src: SrcTransfer<SpaceState>,
}

impl Space {
    pub fn new(
        key: SpaceKey,
        address: ResourceAddress,
        state_src: SrcTransfer<SpaceState>,
    ) -> Result<Self, Error> {
        if address.resource_type != ResourceType::Space {
            Err("expected space address".into())
        } else {
            Ok(Space {
                key: key,
                address: address,
                state_src: state_src,
            })
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SpaceState {
    display: String,
}

impl SpaceState {
    pub fn new(display: &str) -> Self {
        SpaceState {
            display: display.to_string()
        }
    }

    pub fn display(&self) -> String {
        self.display.clone()
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        Ok(bincode::deserialize(bytes)?)
    }
}


/*
impl TryFrom<InitArgs> for SpaceState{
    type Error = Error;

    fn try_from(init_args: InitArgs) -> Result<Self, Self::Error> {
        ResourceKind::Space.init_args_clap_config()?.ok_or("expected init_args for Space")?.validate(&init_args)?;
        let display:String = init_args.args.get("display").cloned().ok_or("expected init arg 'display'")?.try_into()?;
        Ok(Self::new(display.as_str()))
    }
}

 */

impl TryInto<Vec<u8>> for SpaceState {
    type Error = Error;

    fn try_into(self) -> Result<Vec<u8>, Self::Error> {
        Ok(bincode::serialize(&self)?)
    }
}

impl TryInto<Arc<Vec<u8>>> for SpaceState {
    type Error = Error;

    fn try_into(self) -> Result<Arc<Vec<u8>>, Self::Error> {
        Ok(Arc::new(bincode::serialize(&self)?))
    }
}

impl TryFrom<Arc<Vec<u8>>> for SpaceState {
    type Error = Error;

    fn try_from(value: Arc<Vec<u8>>) -> Result<Self, Self::Error> {
        Ok(bincode::deserialize::<SpaceState>(value.as_slice())?)
    }
}

impl TryFrom<Vec<u8>> for SpaceState {
    type Error = Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        Ok(bincode::deserialize::<SpaceState>(value.as_slice())?)
    }
}



