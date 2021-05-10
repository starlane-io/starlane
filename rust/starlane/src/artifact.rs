use crate::keys::SubSpaceKey;
use crate::error::Error;
use serde::{Deserialize, Serialize, Serializer};
use uuid::Uuid;
use std::str::Split;
use crate::actor::{ActorKindExt, ActorKind};
use crate::app::AppKind;

#[derive(Clone,Eq,PartialEq,Hash,Serialize,Deserialize)]
pub struct Artifact
{
    pub id: ArtifactId,
    pub kind: ArtifactKind
}


#[derive(Clone,Eq,PartialEq,Hash,Serialize,Deserialize)]
pub enum ArtifactKind
{
    File,
    AppConfig(AppKind),
    ActorConfig(ActorKind),
    ActorInit(ActorKind),
    Ext(ArtifactKindExt)
}

pub type ArtifactKindExt = Name;

#[derive(Clone,Eq,PartialEq,Hash,Serialize,Deserialize)]
pub struct ArtifactKey
{
    pub sub_space: SubSpaceKey,
    pub id: u64
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
pub struct Name
{
    pub sub_space: SubSpaceName,
    pub path: String,
}


impl Name
{
    pub fn more(string: &str) -> Result<(Self,Split<&str>),Error>
    {
        let (sub_space,mut parts) = SubSpaceName::more(string)?;

        Ok((Name
            {
                sub_space: sub_space,
                path: parts.next().ok_or("sub_space")?.to_string(),
            },parts))
    }

    pub fn from(string: &str) -> Result<Self,Error>
    {
        let (name,_) = Name::more(string)?;
        Ok(name)
    }

    pub fn to(&self) -> String {
        let mut rtn= String::new();
        rtn.push_str(self.sub_space.to().as_str()); rtn.push_str(":");
        rtn.push_str(self.path.as_str());
        return rtn;
    }

    pub fn as_name(&self)->Self
    {
        self.clone()
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

