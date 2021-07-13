use std::collections::HashSet;
use std::convert::{TryFrom, TryInto};
use std::fmt;
use std::iter::FromIterator;
use std::str::{FromStr, Split};

use serde::{Deserialize, Serialize, Serializer};
use uuid::Uuid;

use crate::actor::{ActorKind, ActorSpecific};
use crate::error::Error;
use crate::logger::LogInfo;
use crate::message::Fail;
use crate::message::Fail::ResourceAddressAlreadyInUse;
use crate::names::{Name};
use crate::resource::{ResourceType, ResourceAddressPart, Path, ResourceAddress, SubSpaceKey, ResourceKey, ResourceIdentifier, ArtifactBundleKey, ArtifactAddress, ArtifactBundleAddress, ArtifactKind, ArtifactKey};

pub enum ArtifactIdentifier {
    Key(ArtifactKey),
    Address(ArtifactAddress),
}



#[derive(Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct ArtifactBundle {
    pub domain: String,
    pub space: String,
    pub sub_space: String,
    pub bundle: String,
    pub version: String,
}

impl ArtifactBundle {
    pub fn more(string: &str) -> Result<(Self, Split<&str>), Error> {
        let mut parts = string.split(":");

        Ok((
            ArtifactBundle {
                domain: parts.next().ok_or("hyper")?.to_string(),
                space: parts.next().ok_or("space")?.to_string(),
                sub_space: parts.next().ok_or("sub_space")?.to_string(),
                bundle: parts.next().ok_or("bundle")?.to_string(),
                version: parts.next().ok_or("version")?.to_string(),
            },
            parts,
        ))
    }
}

impl FromStr for ArtifactBundle {
    type Err = Error;

    fn from_str(string: &str) -> Result<Self, Self::Err> {
        let (bundle, _) = ArtifactBundle::more(string)?;
        Ok(bundle)
    }
}

impl ToString for ArtifactBundle {
    fn to_string(&self) -> String {
        let mut rtn = String::new();
        rtn.push_str(self.domain.as_str());
        rtn.push_str(":");
        rtn.push_str(self.space.as_str());
        rtn.push_str(":");
        rtn.push_str(self.sub_space.to_string().as_str());
        rtn.push_str(":");
        rtn.push_str(self.bundle.to_string().as_str());
        rtn.push_str(":");
        rtn.push_str(self.version.to_string().as_str());
        return rtn;
    }
}

#[derive(Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct ArtifactLocation {
    pub bundle: ArtifactBundle,
    pub path: String,
}

impl FromStr for ArtifactLocation {
    type Err = Error;

    fn from_str(string: &str) -> Result<Self, Self::Err> {
        let (bundle, mut parts) = ArtifactBundle::more(string)?;
        let path = parts.next().ok_or("path")?.to_string();
        Ok(ArtifactLocation {
            bundle: bundle,
            path: path,
        })
    }
}

impl ToString for ArtifactLocation {
    fn to_string(&self) -> String {
        let mut rtn = String::new();
        rtn.push_str(self.bundle.to_string().as_str());
        rtn.push_str(":");
        rtn.push_str(self.path.to_string().as_str());
        return rtn;
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct DomainName {
    pub domain: String,
}

impl DomainName {
    pub fn more(string: &str) -> Result<(Self, Split<&str>), Error> {
        let mut parts = string.split(":");

        Ok((
            DomainName {
                domain: parts.next().ok_or("hyper")?.to_string(),
            },
            parts,
        ))
    }

    pub fn from(string: &str) -> Result<Self, Error> {
        let (hyper, _) = DomainName::more(string)?;
        Ok(hyper)
    }

    pub fn to(&self) -> String {
        let mut rtn = String::new();
        rtn.push_str(self.domain.as_str());
        return rtn;
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct SpaceName {
    pub hyper: DomainName,
    pub space: String,
}

impl SpaceName {
    pub fn more(string: &str) -> Result<(Self, Split<&str>), Error> {
        let (hyper, mut parts) = DomainName::more(string)?;

        Ok((
            SpaceName {
                hyper: hyper,
                space: parts.next().ok_or("space")?.to_string(),
            },
            parts,
        ))
    }

    pub fn from(string: &str) -> Result<Self, Error> {
        let (space, _) = SpaceName::more(string)?;
        Ok(space)
    }

    pub fn to(&self) -> String {
        let mut rtn = String::new();
        rtn.push_str(self.hyper.to().as_str());
        rtn.push_str(":");
        rtn.push_str(self.space.as_str());
        return rtn;
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct SubSpaceName {
    pub space: SpaceName,
    pub sub_space: String,
}

impl SubSpaceName {
    pub fn more(string: &str) -> Result<(Self, Split<&str>), Error> {
        let (space, mut parts) = SpaceName::more(string)?;

        Ok((
            SubSpaceName {
                space: space,
                sub_space: parts.next().ok_or("sub_space")?.to_string(),
            },
            parts,
        ))
    }

    pub fn from(string: &str) -> Result<Self, Error> {
        let (sub_space, _) = SubSpaceName::more(string)?;
        Ok(sub_space)
    }

    pub fn to(&self) -> String {
        let mut rtn = String::new();
        rtn.push_str(self.space.to().as_str());
        rtn.push_str(":");
        rtn.push_str(self.sub_space.as_str());
        return rtn;
    }
}



impl Into<ResourceIdentifier> for ArtifactBundleIdentifier {
    fn into(self) -> ResourceIdentifier {
        match self {
            ArtifactBundleIdentifier::Key(key) => ResourceIdentifier::Key(key.into()),
            ArtifactBundleIdentifier::Address(address) => {
                ResourceIdentifier::Address(address.into())
            }
        }
    }
}

pub enum ArtifactBundleIdentifier {
    Key(ArtifactBundleKey),
    Address(ArtifactBundleAddress),
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct ArtifactRef {
    pub address: ArtifactAddress,
    pub kind: ArtifactKind,
}

impl ArtifactRef{
    pub fn new( address: ArtifactAddress, kind: ArtifactKind ) -> Self {
        Self {
            address: address,
            kind: kind
        }
    }
}
