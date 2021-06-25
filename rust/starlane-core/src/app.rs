use std::collections::{HashMap, HashSet};
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

use serde::{Deserialize, Serialize, Serializer};
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::time::Duration;

use crate::actor;
use crate::core::StarCoreCommand;
use crate::error::Error;
use crate::filesystem::File;
use crate::frame::{ChildManagerResourceAction, Reply, StarMessagePayload};
use crate::id::{Id, IdSeq};
use crate::keys::{AppKey, ResourceKey, SubSpaceKey, UserKey};
use crate::message::resource::{ActorMessage, Message, MessageFrom, MessageTo, RawPayload};
use crate::message::{Fail, ProtoStarMessage};
use crate::names::{Name, Specific};
use crate::resource::{
    AssignResourceStateSrc, Labels, Names, ResourceAddress, ResourceAddressPart, ResourceArchetype,
    ResourceAssign, ResourceCreate, ResourceKind, ResourceRecord, ResourceRegistration,
    ResourceStub, ResourceType, SkewerCase,
};
use crate::space::CreateAppControllerFail;
use crate::star::variant::StarVariantCommand;
use crate::star::{
    ActorCreate, CoreAppSequenceRequest, CoreRequest, Empty, LocalResourceLocation, Query, Request,
    ServerCommand, StarComm, StarCommand, StarKey, StarSkel,
};
use std::iter::FromIterator;
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot::error::RecvError;
use tokio::time::error::Elapsed;

#[derive(Clone, Serialize, Deserialize)]
pub enum ConfigSrc {
    None,
    //    Artifact(Artifact)
}

impl ToString for ConfigSrc {
    fn to_string(&self) -> String {
        "None".to_string()
    }
    /*        match self
            {
    //            ConfigSrc::Artifact(artifact) => format!("Artifact::{}",artifact.to_string()),
    //            ConfigSrc::ResourceAddressPart(address) => format!("ResourceAddressPart::{}", address.to_string()),
            }
        }

     */
}

impl FromStr for ConfigSrc {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        unimplemented!()
        /*
                let mut split = s.split("::");
                match split.next().ok_or("nothing to split")?{
                    "Artifact" => Ok(ConfigSrc::Artifact(Artifact::from_str(split.next().ok_or("artifact")?)?)),
        //            "ResourceAddress" => Ok(ConfigSrc::ResourceAddressPart(split.next().ok_or("no more splits")?),
                    what => Err(format!("cannot process ConfigSrc of type {}",what).to_owned().into())
                }
                 */
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
