use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::Error;

use crate::resource::{AssignResourceStateSrc, Labels, Names, ResourceAddress, ResourceArchetype, ResourceAssign, ResourceCreate, ResourceKind, ResourceRecord, ResourceRegistration, ResourceStub, Specific, ArtifactAddress};
use starlane_resources::ResourceIdentifier;
use std::convert::TryInto;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConfigSrc {
    None,
    Artifact(ArtifactAddress)
}

impl ToString for ConfigSrc {
    fn to_string(&self) -> String {
        match self {
            ConfigSrc::None => {
                "None".to_string()
            }
            ConfigSrc::Artifact(address) => {
                let address: ResourceAddress = address.clone().into();
                address.to_string()
            }
        }
    }
}

impl FromStr for ConfigSrc {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if "None" == s {
            Ok(Self::None)
        } else {
            let address = ResourceAddress::from_str(s)?;
            let address: ArtifactAddress = address.try_into()?;
            Ok(Self::Artifact(address))
        }
    }
}

// this is everything describes what an App should be minus it's instance data (instance data like AppKey)
#[derive(Clone, Serialize, Deserialize)]
pub struct AppArchetype {
    pub specific: AppSpecific,
    pub config: ConfigSrc,
}

impl From<AppArchetype> for ResourceArchetype {
    fn from(archetype: AppArchetype) -> Self {
        ResourceArchetype {
            kind: ResourceKind::App,
            specific: Option::Some(archetype.specific),
            config: Option::Some(archetype.config),
        }
    }
}

impl AppArchetype {
    pub fn resource_archetype(self) -> ResourceArchetype {
        ResourceArchetype {
            kind: ResourceKind::App,
            specific: Option::Some(self.specific),
            config: Option::Some(self.config),
        }
    }
}

pub type AppSpecific = Specific;
