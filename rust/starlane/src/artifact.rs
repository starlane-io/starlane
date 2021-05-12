use std::str::{Split, FromStr};

use serde::{Deserialize, Serialize, Serializer};
use uuid::Uuid;

use crate::actor::{ActorKind, ActorSpecific};
use crate::app::AppSpecific;
use crate::error::Error;
use crate::keys::SubSpaceKey;
use crate::names::{Name, Specific};
use std::fmt;

#[derive(Clone,Eq,PartialEq,Hash,Serialize,Deserialize)]
pub struct Artifact
{
    pub id: ArtifactId,
    pub kind: ArtifactKind,
    pub specific: Option<Specific>
}

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

#[derive(Clone,Eq,PartialEq,Hash,Serialize,Deserialize)]
pub struct ArtifactKey
{
    pub sub_space: SubSpaceKey,
    pub id: u64
}

impl fmt::Display for ArtifactKey{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!( f,"[{},{}]",self.sub_space.to_string(),self.id )
    }

}

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

    pub fn from(string: &str) -> Result<Self,Error>
    {
        let (bundle,_) = ArtifactBundle::more(string)?;
        Ok(bundle)
    }

    pub fn to(&self) -> String {
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
pub struct ArtifactId
{
    pub bundle: ArtifactBundle,
    pub path: String
}

impl ArtifactId
{
    pub fn from(string: &str) -> Result<Self,Error>
    {
        let (bundle,mut parts) = ArtifactBundle::more(string)?;
        let path = parts.next().ok_or("path")?.to_string();
        Ok(ArtifactId{
           bundle: bundle,
           path: path
        })
    }

    pub fn to(&self) -> String {
        let mut rtn = String::new();
        rtn.push_str(self.bundle.to().as_str() ); rtn.push_str(":");
        rtn.push_str(self.path.to_string().as_str());
        return rtn;
    }
}

#[derive(Clone,Eq,PartialEq,Hash,Serialize,Deserialize)]
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

#[derive(Clone,Eq,PartialEq,Hash,Serialize,Deserialize)]
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

#[derive(Clone,Eq,PartialEq,Hash,Serialize,Deserialize)]
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

