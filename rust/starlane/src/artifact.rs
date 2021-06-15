use std::str::{Split, FromStr};

use serde::{Deserialize, Serialize, Serializer};
use uuid::Uuid;

use crate::actor::{ActorKind, ActorSpecific};
use crate::error::Error;
use crate::keys::{SubSpaceKey, ResourceKey};
use crate::names::{Name, Specific};
use std::fmt;
use crate::resource::{ResourceAddress, ResourceType, ArtifactBundleKind, ResourceIdentifier, Path, ResourceAddressPart};
use std::convert::{TryFrom, TryInto};
use crate::message::Fail;
use std::collections::HashSet;
use std::iter::FromIterator;
use crate::logger::LogInfo;
use crate::message::Fail::ResourceAddressAlreadyInUse;
/*
#[derive(Clone,Eq,PartialEq,Hash,Serialize,Deserialize)]
pub struct Artifact
{
    pub location: ArtifactLocation,
    pub kind: ArtifactKind,
    pub specific: Option<Specific>
}

impl Artifact
{
    fn to_address(&self) -> Result<ResourceAddress,Error>{
       ResourceType::Artifact.address_structure().from_str(self.to_string().as_str() )
    }
}

impl FromStr for Artifact{
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut split = s.split("::");
        let location = ArtifactLocation::from_str(split.next().ok_or("missing location")? )?;
        let kind = ArtifactKind::from_str(split.next().ok_or("missing kind")? )?;
        let specific = if let Option::Some(specific) = split.next(){
            Option::Some(Specific::from_str(specific)?)
        } else {
            Option::None
        };
        Ok(Artifact{
            location: location,
            kind: kind,
            specific: specific
        })
    }
}

impl ToString for Artifact{
    fn to_string(&self) -> String {
       let mut rtn = String::new();
       rtn.push_str(self.location.to_string().as_str() );
       rtn.push_str("::");
       rtn.push_str(self.kind.to_string().as_str() );
       if let Option::Some(specific) = &self.specific{
           rtn.push_str("::");
           rtn.push_str(specific.to_string().as_str() );
       }
       rtn
    }
}



 */

#[derive(Clone,Eq,PartialEq,Hash,Serialize,Deserialize)]
pub enum ArtifactKind
{
    File,
    AppConfig,
    ActorConfig,
    ActorInit
}

impl fmt::Display for ArtifactKind{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!( f,"{}",
                match self{
                    ArtifactKind::File => "File".to_string(),
                    ArtifactKind::AppConfig => "AppConfig".to_string(),
                    ArtifactKind::ActorConfig => "ActorConfig".to_string(),
                    ArtifactKind::ActorInit => "ActorInit".to_string(),
                })
    }
}

impl FromStr for ArtifactKind
{
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s
        {
            "File" => Ok(ArtifactKind::File),
            "AppConfig" => Ok(ArtifactKind::AppConfig),
            "ActorConfig" => Ok(ArtifactKind::ActorConfig),
            "ActorInit" => Ok(ArtifactKind::ActorInit),
            _ => Err(format!("could not find ArtifactKind: {}",s).into())
        }
    }
}


pub type ArtifactKindExt = Name;


#[derive(Debug,Clone,Eq,PartialEq,Hash,Serialize,Deserialize)]
pub struct ArtifactBundleKey
{
    pub sub_space: SubSpaceKey,
    pub id: ArtifactBundleId
}

impl Into<ResourceKey> for ArtifactBundleKey{
    fn into(self) -> ResourceKey {
        ResourceKey::ArtifactBundle(self)
    }
}

impl ArtifactBundleKey{
    pub fn new( sub_space: SubSpaceKey, id: ArtifactBundleId )->Self{
        ArtifactBundleKey{
            sub_space: sub_space,
            id: id
        }
    }
}

#[derive(Clone,Hash,Eq,PartialEq)]
pub struct Artifact {
    address: ResourceAddress
}


impl Artifact {
    pub fn parent(&self)->ArtifactBundleResourceAddress {
        return ArtifactBundleResourceAddress{
            address: self.address.parent().expect("artifact should have bundle parent")
        }
    }

    pub fn dir(&self)->Result<Option<Path>,Error>{
        if let Option::Some(ResourceAddressPart::Path(path)) = self.address.last() {
            Ok(path.clone().parent())
        }
        else{
            Err("expected ArtifactResourceAddress to end in a Path".into())
        }
    }

    pub fn path(&self)->Result<Path,Error>{
        if let Option::Some(ResourceAddressPart::Path(path)) = self.address.last() {
            Ok(path.clone())
        }
        else{
            Err("expected ArtifactResourceAddress to end in a Path".into())
        }
    }
}


impl FromStr for Artifact {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {

        if s.contains("::<") {
            let address = ResourceAddress::from_str(s)?;
            let artifact = address.try_into()?;
            Ok(artifact)
        } else {
            let mut string = String::new();
            string.push_str(s);
            string.push_str("::<Artifact>");
            let address = ResourceAddress::from_str(string.as_str())?;
            let artifact = address.try_into()?;
            Ok(artifact)
        }
    }
}

impl ToString for Artifact {
    fn to_string(&self) -> String {
        self.address.to_string()
    }
}

impl LogInfo for Artifact {
    fn log_identifier(&self) -> String {
        let address: ResourceAddress = self.clone().into();
        address.to_parts_string()
    }

    fn log_kind(&self) -> String {
        let address: ResourceAddress = self.clone().into();
        address.resource_type().to_string()
    }

    fn log_object(&self) -> String {
        "ArtifactResourceAddress".to_string()
    }
}



impl Into<ResourceAddress> for Artifact {
    fn into(self) -> ResourceAddress {
        self.address
    }
}

impl TryFrom<ResourceAddress> for Artifact {
    type Error = Fail;

    fn try_from(value: ResourceAddress) -> Result<Self, Self::Error> {
        if value.resource_type() != ResourceType::Artifact {
            Err(Fail::WrongResourceType {expected:HashSet::from_iter(vec![ResourceType::Artifact]),received: value.resource_type()})
        } else {
            Ok(Artifact {
                address: value
            })
        }
    }
}

pub enum ArtifactIdentifier{
    Key(ArtifactKey),
    Address(Artifact)
}


#[derive(Debug,Clone,Eq,PartialEq,Hash,Serialize,Deserialize)]
pub struct ArtifactKey
{
    pub bundle: ArtifactBundleKey,
    pub id: ArtifactId
}

impl ArtifactKey{
    pub fn new( bundle: ArtifactBundleKey, id: ArtifactId )->Self{
        ArtifactKey{
            bundle: bundle,
            id: id
        }
    }
}

impl ToString for ArtifactKey{
    fn to_string(&self) -> String {
        format!("{}-{}",self.bundle.to_string(), self.id.to_string())
    }
}

impl ToString for ArtifactBundleKey{
    fn to_string(&self) -> String {
        format!("{}-{}",self.sub_space.to_string(), self.id.to_string())
    }
}


impl FromStr for ArtifactBundleKey{
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let pos = s.rfind( '-').ok_or("expected '-' between parent and id")?;
        let (parent,id)= s.split_at(pos);
        let sub_space= SubSpaceKey::from_str(parent)?;
        let id = ArtifactBundleId::from_str(id)?;
        Ok(ArtifactBundleKey{
            sub_space: sub_space,
            id: id
        })
    }
}

impl FromStr for ArtifactKey{
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let pos = s.rfind( '-').ok_or("expected '-' between parent and id")?;
        let (parent,id)= s.split_at(pos);
        let bundle = ArtifactBundleKey::from_str(parent)?;
        let id = ArtifactId::from_str(id)?;
        Ok(ArtifactKey{
            bundle: bundle,
            id: id
        })
    }
}

pub type ArtifactBundleId = u64;
pub type ArtifactId = u32;


#[derive(Clone,Eq,PartialEq,Hash,Serialize,Deserialize)]
pub struct NameKey
{
    pub sub_space: SubSpaceKey,
    pub id: u64
}

#[derive(Clone,Eq,PartialEq,Hash,Serialize,Deserialize)]
pub struct ArtifactBundle
{
    pub domain: String,
    pub space: String,
    pub sub_space: String,
    pub bundle: String,
    pub version: String,
}

impl ArtifactBundle
{
    pub fn more(string: &str) -> Result<(Self,Split<&str>),Error>
    {
        let mut parts = string.split(":");

        Ok((ArtifactBundle
        {
            domain: parts.next().ok_or("hyper")?.to_string(),
            space: parts.next().ok_or("space")?.to_string(),
            sub_space: parts.next().ok_or("sub_space")?.to_string(),
            bundle: parts.next().ok_or("bundle")?.to_string(),
            version: parts.next().ok_or("version")?.to_string()
        },parts))
    }
}

impl FromStr for ArtifactBundle
{
    type Err = Error;

    fn from_str(string: &str) -> Result<Self,Self::Err>
    {
        let (bundle,_) = ArtifactBundle::more(string)?;
        Ok(bundle)
    }
}


impl ToString for ArtifactBundle{
    fn to_string(&self) -> String {
        let mut rtn = String::new();
        rtn.push_str(self.domain.as_str()); rtn.push_str(":");
        rtn.push_str(self.space.as_str()); rtn.push_str(":");
        rtn.push_str(self.sub_space.to_string().as_str()); rtn.push_str(":");
        rtn.push_str(self.bundle.to_string().as_str()); rtn.push_str(":");
        rtn.push_str(self.version.to_string().as_str());
        return rtn;
    }
}

#[derive(Clone,Eq,PartialEq,Hash,Serialize,Deserialize)]
pub struct ArtifactLocation
{
    pub bundle: ArtifactBundle,
    pub path: String
}



impl FromStr for ArtifactLocation
{
    type Err = Error;

    fn from_str(string: &str) -> Result<Self,Self::Err>
    {
        let (bundle,mut parts) = ArtifactBundle::more(string)?;
        let path = parts.next().ok_or("path")?.to_string();
        Ok(ArtifactLocation {
           bundle: bundle,
           path: path
        })
    }
}

impl ToString for ArtifactLocation{
    fn to_string(&self) -> String {
        let mut rtn = String::new();
        rtn.push_str(self.bundle.to_string().as_str() ); rtn.push_str(":");
        rtn.push_str(self.path.to_string().as_str());
        return rtn;
    }
}

#[derive(Debug,Clone,Eq,PartialEq,Hash,Serialize,Deserialize)]
pub struct DomainName
{
    pub domain: String
}

impl DomainName
{
    pub fn more(string: &str) -> Result<(Self,Split<&str>),Error>
    {
        let mut parts = string.split(":");

        Ok((DomainName
            {
                domain: parts.next().ok_or("hyper")?.to_string(),
            }, parts))
    }

    pub fn from(string: &str) -> Result<Self,Error>
    {
        let (hyper,_) = DomainName::more(string)?;
        Ok(hyper)
    }

    pub fn to(&self) -> String {
        let mut rtn = String::new();
        rtn.push_str(self.domain.as_str());
        return rtn;
    }
}

#[derive(Debug,Clone,Eq,PartialEq,Hash,Serialize,Deserialize)]
pub struct SpaceName
{
    pub hyper: DomainName,
    pub space: String
}

impl SpaceName
{
    pub fn more(string: &str) -> Result<(Self,Split<&str>),Error>
    {
        let (hyper,mut parts) = DomainName::more(string)?;

        Ok((SpaceName
            {
                hyper: hyper,
                space: parts.next().ok_or("space")?.to_string(),
            },parts))
    }

    pub fn from(string: &str) -> Result<Self,Error>
    {
        let (space,_) = SpaceName::more(string)?;
        Ok(space)
    }

    pub fn to(&self) -> String {
        let mut rtn= String::new();
        rtn.push_str(self.hyper.to().as_str()); rtn.push_str(":");
        rtn.push_str(self.space.as_str());
        return rtn;
    }
}

#[derive(Debug,Clone,Eq,PartialEq,Hash,Serialize,Deserialize)]
pub struct SubSpaceName
{
    pub space: SpaceName,
    pub sub_space: String
}

impl SubSpaceName
{
    pub fn more(string: &str) -> Result<(Self,Split<&str>),Error>
    {
        let (space,mut parts) = SpaceName::more(string)?;

        Ok((SubSpaceName
            {
                space: space,
                sub_space: parts.next().ok_or("sub_space")?.to_string(),
            },parts))
    }

    pub fn from(string: &str) -> Result<Self,Error>
    {
        let (sub_space,_) = SubSpaceName::more(string)?;
        Ok(sub_space)
    }

    pub fn to(&self) -> String {
        let mut rtn= String::new();
        rtn.push_str(self.space.to().as_str()); rtn.push_str(":");
        rtn.push_str(self.sub_space.as_str());
        return rtn;
    }
}


#[derive(Debug,Clone,Serialize,Deserialize,Eq,PartialEq,Hash)]
pub struct ArtifactBundleResourceAddress{
    address: ResourceAddress
}

impl Into<ArtifactBundleIdentifier> for ArtifactBundleResourceAddress {
    fn into(self) -> ArtifactBundleIdentifier {
        ArtifactBundleIdentifier::Address(self)
    }
}



impl Into<ResourceAddress> for ArtifactBundleResourceAddress{
    fn into(self) -> ResourceAddress {
        self.address
    }
}

impl TryFrom<ResourceAddress> for ArtifactBundleResourceAddress {
    type Error = Fail;

    fn try_from(value: ResourceAddress) -> Result<Self, Self::Error> {
        if value.resource_type() != ResourceType::ArtifactBundle {
            Err(Fail::WrongResourceType {expected:HashSet::from_iter(vec![ResourceType::ArtifactBundle]),received: value.resource_type()})
        } else {
            Ok(ArtifactBundleResourceAddress{
                address: value
            })
        }
    }
}


impl Into<ResourceIdentifier> for ArtifactBundleIdentifier {
    fn into(self) -> ResourceIdentifier {
        match self {
            ArtifactBundleIdentifier::Key(key) => {
                ResourceIdentifier::Key(key.into())
            }
            ArtifactBundleIdentifier::Address(address) => {
                ResourceIdentifier::Address(address.into())
            }
        }
    }
}


pub enum ArtifactBundleIdentifier{
    Key(ArtifactBundleKey),
    Address(ArtifactBundleResourceAddress)
}
