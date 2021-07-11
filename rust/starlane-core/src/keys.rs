use std::collections::HashSet;
use std::fmt;
use std::iter::FromIterator;
use std::str::FromStr;

use bincode::deserialize;
use serde::{Deserialize, Serialize, Serializer};
use uuid::Uuid;

use crate::actor::ActorKey;
use crate::artifact::{ArtifactBundleId, ArtifactBundleKey, ArtifactId, ArtifactKey};
use crate::error::Error;
use crate::frame::Reply;
use crate::id::Id;
use crate::keys::ResourceId::UrlPathPattern;
use crate::message::Fail;
use crate::names::Name;
use crate::permissions::{Priviledges, User, UserKind};
use crate::resource::{Labels, ResourceArchetype, ResourceAssign, ResourceIdentifier, ResourceKind, ResourceManagerKey, ResourceRecord, ResourceSelectorId, ResourceStub, ResourceType};
use crate::resource::address::{ResourceAddressPart, SkewerCase};

pub type SpaceId = u32;

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub enum SpaceKey {
    HyperSpace,
    Space(SpaceId),
}

impl SpaceKey {
    pub fn hyper_space() -> Self {
        Self::from_index(0)
    }

    pub fn from_index(index: u32) -> Self {
        if index == 0 {
            SpaceKey::HyperSpace
        } else {
            SpaceKey::Space(index)
        }
    }

    pub fn id(&self) -> SpaceId {
        match self {
            SpaceKey::HyperSpace => 0,
            SpaceKey::Space(index) => index.clone(),
        }
    }
}

impl FromStr for SpaceKey {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(SpaceKey::from_index(SpaceId::from_str(s)?))
    }
}

impl ToString for SpaceKey {
    fn to_string(&self) -> String {
        self.id().to_string()
    }
}

pub type UserId = i32;

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub struct UserKey {
    pub space: SpaceKey,
    pub id: UserId,
}

impl UserKey {
    pub fn bin(&self) -> Result<Vec<u8>, Error> {
        let mut bin = bincode::serialize(self)?;
        Ok(bin)
    }

    pub fn from_bin(mut bin: Vec<u8>) -> Result<Self, Error> {
        let mut key = bincode::deserialize::<Self>(bin.as_slice())?;
        Ok(key)
    }
}

impl UserKey {
    pub fn new(space: SpaceKey, id: UserId) -> Self {
        UserKey { space, id: id }
    }

    pub fn hyper_user() -> Self {
        UserKey::new(SpaceKey::HyperSpace, 0)
    }

    pub fn super_user(space: SpaceKey) -> Self {
        UserKey::new(space, 0)
    }

    pub fn is_hyperuser(&self) -> bool {
        match self.space {
            SpaceKey::HyperSpace => match self.id {
                0 => true,
                _ => false,
            },
            _ => false,
        }
    }

    pub fn privileges(&self) -> Priviledges {
        if self.is_hyperuser() {
            Priviledges::all()
        } else {
            Priviledges::new()
        }
    }
}

impl ToString for UserKey {
    fn to_string(&self) -> String {
        format!("{}-{}", self.space.to_string(), self.id.to_string())
    }
}

impl FromStr for UserKey {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let pos = s.rfind('-').ok_or("expected '-' between parent and id")?;
        let (parent, id) = s.split_at(pos);
        let space = SpaceKey::from_str(parent)?;
        let id = UserId::from_str(id)?;
        Ok(UserKey {
            space: space,
            id: id,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct SubSpaceKey {
    pub space: SpaceKey,
    pub id: SubSpaceId,
}

impl SubSpaceKey {
    pub fn hyper_default() -> Self {
        SubSpaceKey::new(SpaceKey::HyperSpace, 0)
    }

    pub fn new(space: SpaceKey, id: SubSpaceId) -> Self {
        SubSpaceKey {
            space: space,
            id: id,
        }
    }
}

impl Into<ResourceKey> for SubSpaceKey {
    fn into(self) -> ResourceKey {
        ResourceKey::SubSpace(self)
    }
}

pub type SubSpaceId = u32;

impl ToString for SubSpaceKey {
    fn to_string(&self) -> String {
        format!("{}-{}", self.space.to_string(), self.id.to_string())
    }
}

impl FromStr for SubSpaceKey {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let pos = s.rfind('-').ok_or("expected '-' between parent and id")?;
        let (parent, id) = s.split_at(pos);
        let mut id = id.to_string();
        id.remove(0);
        let space = SpaceKey::from_str(parent)?;
        let id = SubSpaceId::from_str(id.as_str())?;
        Ok(SubSpaceKey {
            space: space,
            id: id,
        })
    }
}

#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct AppKey {
    pub sub_space: SubSpaceKey,
    pub id: AppId,
}

impl AppKey {
    pub fn address_part(&self) -> Result<ResourceAddressPart, Error> {
        Ok(ResourceAddressPart::SkewerCase(SkewerCase::new(
            self.id.to_string().as_str(),
        )?))
    }
}

impl AppKey {
    pub fn bin(&self) -> Result<Vec<u8>, Error> {
        let mut bin = bincode::serialize(self)?;
        Ok(bin)
    }

    pub fn from_bin(mut bin: Vec<u8>) -> Result<AppKey, Error> {
        let mut key = bincode::deserialize::<AppKey>(bin.as_slice())?;
        Ok(key)
    }
}

impl Into<ResourceKey> for AppKey {
    fn into(self) -> ResourceKey {
        ResourceKey::App(self)
    }
}

pub type AppId = u64;

impl AppKey {
    pub fn new(sub_space: SubSpaceKey, id: AppId) -> Self {
        AppKey {
            sub_space: sub_space,
            id: id,
        }
    }
}

impl ToString for AppKey {
    fn to_string(&self) -> String {
        format!("{}-{}", self.sub_space.to_string(), self.id.to_string())
    }
}

impl FromStr for AppKey {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let pos = s.rfind('-').ok_or("expected '-' between parent and id")?;
        let (parent, id) = s.split_at(pos);
        let sub_space = SubSpaceKey::from_str(parent)?;
        let id = AppId::from_str(id)?;
        Ok(AppKey {
            sub_space: sub_space,
            id: id,
        })
    }
}

pub type MessageId = Uuid;
pub type DomainId = u32;
pub type UrlPathPatternId = u64;
pub type ProxyId = u64;
pub type DatabaseId = u32;

#[derive(Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub enum ResourceId {
    Root,
    Space(u32),
    SubSpace(SubSpaceId),
    App(AppId),
    Actor(Id),
    User(UserId),
    File(FileId),
    FileSystem(FileSystemId),
    Domain(DomainId),
    UrlPathPattern(UrlPathPatternId),
    Proxy(ProxyId),
    ArtifactBundle(ArtifactBundleId),
    Artifact(ArtifactId),
    Database(DatabaseId),
}

impl ResourceId {
    pub fn resource_type(&self) -> ResourceType {
        match self {
            ResourceId::Root => ResourceType::Root,
            ResourceId::Space(_) => ResourceType::Space,
            ResourceId::SubSpace(_) => ResourceType::SubSpace,
            ResourceId::App(_) => ResourceType::App,
            ResourceId::Actor(_) => ResourceType::Actor,
            ResourceId::User(_) => ResourceType::User,
            ResourceId::File(_) => ResourceType::File,
            ResourceId::FileSystem(_) => ResourceType::FileSystem,
            ResourceId::Domain(_) => ResourceType::Domain,
            ResourceId::UrlPathPattern(_) => ResourceType::UrlPathPattern,
            ResourceId::Proxy(_) => ResourceType::Proxy,
            ResourceId::ArtifactBundle(_) => ResourceType::ArtifactBundle,
            ResourceId::Artifact(_) => ResourceType::Artifact,
            ResourceId::Database(_) => ResourceType::Database,
        }
    }

    pub fn new(resource_type: &ResourceType, id: Id) -> Self {
        match resource_type {
            ResourceType::Root => Self::Root,
            ResourceType::Space => Self::Space(id.index as _),
            ResourceType::SubSpace => Self::SubSpace(id.index as _),
            ResourceType::App => Self::App(id.index as _),
            ResourceType::Actor => Self::Actor(id),
            ResourceType::User => Self::User(id.index as _),
            ResourceType::FileSystem => Self::FileSystem(id.index as _),
            ResourceType::File => Self::File(id.index as _),
            ResourceType::Domain => Self::Domain(id.index as _),
            ResourceType::UrlPathPattern => Self::UrlPathPattern(id.index as _),
            ResourceType::Proxy => Self::Proxy(id.index as _),
            ResourceType::ArtifactBundle => Self::ArtifactBundle(id.index as _),
            ResourceType::Artifact => Self::Artifact(id.index as _),
            ResourceType::Database => Self::Database(id.index as _),
        }
    }
}

impl ToString for ResourceId {
    fn to_string(&self) -> String {
        match self {
            ResourceId::Root => "Root".to_string(),
            ResourceId::Space(id) => id.to_string(),
            ResourceId::SubSpace(id) => id.to_string(),
            ResourceId::App(id) => id.to_string(),
            ResourceId::Actor(id) => id.to_string(),
            ResourceId::User(id) => id.to_string(),
            ResourceId::File(id) => id.to_string(),
            ResourceId::FileSystem(id) => id.to_string(),
            ResourceId::Domain(id) => id.to_string(),
            ResourceId::UrlPathPattern(id) => id.to_string(),
            ResourceId::Proxy(id) => id.to_string(),
            ResourceId::ArtifactBundle(id) => id.to_string(),
            ResourceId::Artifact(id) => id.to_string(),
            ResourceId::Database(id) => id.to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub enum ResourceKey {
    Root,
    Space(SpaceKey),
    SubSpace(SubSpaceKey),
    App(AppKey),
    Actor(ActorKey),
    User(UserKey),
    File(FileKey),
    FileSystem(FileSystemKey),
    Domain(DomainKey),
    UrlPathPattern(UrlPathPatternKey),
    Proxy(ProxyKey),
    ArtifactBundle(ArtifactBundleKey),
    Artifact(ArtifactKey),
    Database(DatabaseKey),
}

impl ResourceKey {
    pub fn new(parent: ResourceKey, id: ResourceId) -> Result<Self, Error> {
        match id {
            ResourceId::Root => Ok(Self::Root),
            ResourceId::Space(id) => Ok(Self::Space(SpaceKey::from_index(id))),
            ResourceId::SubSpace(index) => {
                if let Self::Space(parent) = parent {
                    Ok(Self::SubSpace(SubSpaceKey::new(parent, index)))
                } else {
                    Err(format!(
                        "mismatched types! parent {} is not compatible with id: {}",
                        parent,
                        id.to_string()
                    )
                    .into())
                }
            }
            ResourceId::App(index) => {
                if let Self::SubSpace(parent) = parent {
                    Ok(Self::App(AppKey::new(parent, index)))
                } else {
                    Err(format!(
                        "mismatched types! parent {} is not compatible with id: {}",
                        parent,
                        id.to_string()
                    )
                    .into())
                }
            }
            ResourceId::Actor(index) => {
                if let Self::App(parent) = parent {
                    Ok(Self::Actor(ActorKey::new(parent, index)))
                } else {
                    Err(format!(
                        "mismatched types! parent {} is not compatible with id: {}",
                        parent,
                        id.to_string()
                    )
                    .into())
                }
            }
            ResourceId::User(index) => {
                if let Self::Space(parent) = parent {
                    Ok(Self::User(UserKey::new(parent, index)))
                } else {
                    Err(format!(
                        "mismatched types! parent {} is not compatible with id: {}",
                        parent,
                        id.to_string()
                    )
                    .into())
                }
            }

            ResourceId::File(index) => {
                if let Self::FileSystem(parent) = parent {
                    Ok(Self::File(FileKey::new(parent, index)))
                } else {
                    Err(format!(
                        "mismatched types! parent {} is not compatible with id: {}",
                        parent,
                        id.to_string()
                    )
                    .into())
                }
            }
            ResourceId::FileSystem(index) => {
                if let Self::SubSpace(parent) = parent {
                    Ok(Self::FileSystem(FileSystemKey::SubSpace(
                        SubSpaceFilesystemKey {
                            sub_space: parent,
                            id: index,
                        },
                    )))
                } else if let Self::App(parent) = parent {
                    Ok(Self::FileSystem(FileSystemKey::App(AppFilesystemKey {
                        app: parent,
                        id: index,
                    })))
                } else {
                    Err(format!(
                        "mismatched types! parent {} is not compatible with id: {}",
                        parent,
                        id.to_string()
                    )
                    .into())
                }
            }
            ResourceId::Domain(index) => {
                if let Self::Space(parent) = parent {
                    Ok(Self::Domain(DomainKey {
                        space: parent,
                        id: index,
                    }))
                } else {
                    Err(format!(
                        "mismatched types! parent {} is not compatible with id: {}",
                        parent,
                        id.to_string()
                    )
                    .into())
                }
            }
            ResourceId::UrlPathPattern(index) => {
                if let Self::Domain(parent) = parent {
                    Ok(Self::UrlPathPattern(UrlPathPatternKey {
                        domain: parent,
                        id: index,
                    }))
                } else {
                    Err(format!(
                        "mismatched types! parent {} is not compatible with id: {}",
                        parent,
                        id.to_string()
                    )
                    .into())
                }
            }
            ResourceId::Proxy(index) => {
                if let Self::Space(parent) = parent {
                    Ok(Self::Proxy(ProxyKey {
                        space: parent,
                        id: index,
                    }))
                } else {
                    Err(format!(
                        "mismatched types! parent {} is not compatible with id: {}",
                        parent,
                        id.to_string()
                    )
                    .into())
                }
            }
            ResourceId::ArtifactBundle(index) => {
                if let Self::SubSpace(parent) = parent {
                    Ok(Self::ArtifactBundle(ArtifactBundleKey {
                        sub_space: parent,
                        id: index,
                    }))
                } else {
                    Err(format!(
                        "mismatched types! parent {} is not compatible with id: {}",
                        parent,
                        id.to_string()
                    )
                    .into())
                }
            }
            ResourceId::Artifact(index) => {
                if let Self::ArtifactBundle(parent) = parent {
                    Ok(Self::Artifact(ArtifactKey {
                        bundle: parent,
                        id: index,
                    }))
                } else {
                    Err(format!(
                        "mismatched types! parent {} is not compatible with id: {}",
                        parent,
                        id.to_string()
                    )
                    .into())
                }
            }
            ResourceId::Database(index) => {
                if let Self::SubSpace(parent) = parent {
                    Ok(Self::Database(DatabaseKey::SubSpace(
                        SubKey{
                            parent: parent,
                            id: index,
                        },
                    )))
                } else if let Self::App(parent) = parent {
                    Ok(Self::Database(DatabaseKey::App(
                        SubKey{
                            parent: parent,
                            id: index,
                        },
                    )))
                } else {
                    Err(format!(
                        "mismatched types! parent {} is not compatible with id: {}",
                        parent,
                        id.to_string()
                    )
                    .into())
                }
            }
        }
    }

    pub fn to_snake_case(self) -> String {
        self.to_string().replace("-", "_")
    }

    pub fn to_skewer_case(self) -> String {
        self.to_string().replace("_", "-")
    }


    pub fn id(&self) -> ResourceId {
        match self {
            ResourceKey::Root => ResourceId::Root,
            ResourceKey::Space(space) => ResourceId::Space(space.id()),
            ResourceKey::SubSpace(sub_space) => ResourceId::SubSpace(sub_space.id.clone()),
            ResourceKey::App(app) => ResourceId::App(app.id.clone()),
            ResourceKey::Actor(actor) => ResourceId::Actor(actor.id.clone()),
            ResourceKey::User(user) => ResourceId::User(user.id.clone()),
            ResourceKey::File(file) => ResourceId::File(file.id.clone()),
            ResourceKey::FileSystem(filesystem) => filesystem.id(),
            ResourceKey::Domain(domain) => ResourceId::Domain(domain.id.clone()),
            ResourceKey::UrlPathPattern(pattern) => ResourceId::UrlPathPattern(pattern.id.clone()),
            ResourceKey::Proxy(proxy) => ResourceId::Proxy(proxy.id.clone()),
            ResourceKey::ArtifactBundle(bundle) => ResourceId::ArtifactBundle(bundle.id.clone()),
            ResourceKey::Artifact(artifact) => ResourceId::Artifact(artifact.id.clone()),
            ResourceKey::Database(database) =>  database.id(),
        }
    }

    pub fn generate_address_tail(&self) -> Result<String, Fail> {
        match self {
            ResourceKey::Root => Err(Fail::ResourceCannotGenerateAddress),
            ResourceKey::Space(_) => Err(Fail::ResourceCannotGenerateAddress),
            ResourceKey::SubSpace(_) => Err(Fail::ResourceCannotGenerateAddress),
            ResourceKey::App(app) => Ok(app.id.to_string()),
            ResourceKey::Actor(actor) => Ok(actor.id.to_string()),
            ResourceKey::User(user) => Err(Fail::ResourceCannotGenerateAddress),
            ResourceKey::File(_) => Err(Fail::ResourceCannotGenerateAddress),
            ResourceKey::FileSystem(filesystem) => match filesystem {
                FileSystemKey::App(app) => Ok(app.id.to_string()),
                FileSystemKey::SubSpace(sub) => Ok(sub.id.to_string()),
            },
            ResourceKey::Domain(domain) => Err(Fail::ResourceCannotGenerateAddress),
            ResourceKey::UrlPathPattern(_) => Err(Fail::ResourceCannotGenerateAddress),
            ResourceKey::Proxy(_) => Err(Fail::ResourceCannotGenerateAddress),
            ResourceKey::ArtifactBundle(_) => Err(Fail::ResourceCannotGenerateAddress),
            ResourceKey::Artifact(_) => Err(Fail::ResourceCannotGenerateAddress),
            ResourceKey::Database(_) =>  Err(Fail::ResourceCannotGenerateAddress),
        }
    }

    pub fn parent(&self) -> Option<ResourceKey> {
        match self {
            ResourceKey::Root => Option::None,
            ResourceKey::Space(_) => Option::Some(ResourceKey::Root),
            ResourceKey::SubSpace(sub_space) => {
                Option::Some(ResourceKey::Space(sub_space.space.clone()))
            }
            ResourceKey::App(app) => Option::Some(ResourceKey::SubSpace(app.sub_space.clone())),
            ResourceKey::Actor(actor) => Option::Some(ResourceKey::App(actor.app.clone())),
            ResourceKey::User(user) => Option::Some(ResourceKey::Space(user.space.clone())),
            ResourceKey::File(file) => {
                Option::Some(ResourceKey::FileSystem(file.filesystem.clone()))
            }
            ResourceKey::FileSystem(filesystem) => match filesystem {
                FileSystemKey::App(app) => Option::Some(ResourceKey::App(app.app.clone())),
                FileSystemKey::SubSpace(sub_space) => {
                    Option::Some(ResourceKey::SubSpace(sub_space.sub_space.clone()))
                }
            },
            ResourceKey::Domain(domain) => Option::Some(ResourceKey::Space(domain.space.clone())),
            ResourceKey::UrlPathPattern(pattern) => {
                Option::Some(ResourceKey::Domain(pattern.domain.clone()))
            }
            ResourceKey::Proxy(proxy) => Option::Some(ResourceKey::Space(proxy.space.clone())),

            ResourceKey::ArtifactBundle(bundle) => {
                Option::Some(ResourceKey::SubSpace(bundle.sub_space.clone()))
            }
            ResourceKey::Artifact(artifact) => {
                Option::Some(ResourceKey::ArtifactBundle(artifact.bundle.clone()))
            }
            ResourceKey::Database(filesystem) => match filesystem {
                DatabaseKey::App(app) => Option::Some(ResourceKey::App(app.parent.clone())),
                DatabaseKey::SubSpace(sub_space) => {
                    Option::Some(ResourceKey::SubSpace(sub_space.parent.clone()))
                }
            },
        }
    }

    pub fn space(&self) -> Result<SpaceKey, Fail> {
        match self {
            ResourceKey::Root => Err(Fail::WrongResourceType {
                expected: HashSet::from_iter(vec![
                    ResourceType::Space,
                    ResourceType::SubSpace,
                    ResourceType::App,
                    ResourceType::Actor,
                    ResourceType::User,
                    ResourceType::FileSystem,
                    ResourceType::File,
                ]),
                received: ResourceType::Root,
            }),
            ResourceKey::Space(space) => Ok(space.clone()),
            ResourceKey::SubSpace(sub_space) => Ok(sub_space.space.clone()),
            ResourceKey::App(app) => Ok(app.sub_space.space.clone()),
            ResourceKey::Actor(actor) => Ok(actor.app.sub_space.space.clone()),
            ResourceKey::User(user) => Ok(user.space.clone()),
            ResourceKey::File(file) => Ok(match &file.filesystem {
                FileSystemKey::App(app) => app.app.sub_space.space.clone(),
                FileSystemKey::SubSpace(sub_space) => sub_space.sub_space.space.clone(),
            }),
            ResourceKey::FileSystem(filesystem) => Ok(match filesystem {
                FileSystemKey::App(app) => app.app.sub_space.space.clone(),
                FileSystemKey::SubSpace(sub_space) => sub_space.sub_space.space.clone(),
            }),
            ResourceKey::Domain(domain) => Ok(domain.space.clone()),
            ResourceKey::UrlPathPattern(pattern) => Ok(pattern.domain.space.clone()),
            ResourceKey::Proxy(proxy) => Ok(proxy.space.clone()),
            ResourceKey::ArtifactBundle(bundle) => Ok(bundle.sub_space.space.clone()),
            ResourceKey::Artifact(artifact) => Ok(artifact.bundle.sub_space.space.clone()),
            ResourceKey::Database(filesystem) => Ok(match filesystem {
                DatabaseKey::App(app) => app.parent.sub_space.space.clone(),
                DatabaseKey::SubSpace(sub_space) => sub_space.parent.space.clone(),
            }),
        }
    }

    /*    pub fn sub_space(&self)->Result<SubSpaceKey,Fail> {
           match self{
               ResourceKey::SubSpace(sub_space) => Ok(sub_space.clone()),
               ResourceKey::App(app) => Ok(app.sub_space.clone()),
               ResourceKey::Actor(actor) => Ok(actor.app.sub_space.clone()),
               ResourceKey::Artifact(artifact) => Ok(artifact.sub_space.clone()),
               ResourceKey::File(file) => Ok(match &file.filesystem{
                   FileSystemKey::App(app) => app.app.sub_space.clone(),
                   FileSystemKey::SubSpace(sub_space) => sub_space.sub_space.clone(),
               }),
               ResourceKey::FileSystem(filesystem) => {
                   Ok(match filesystem{
                       FileSystemKey::App(app) => app.app.sub_space.clone(),
                       FileSystemKey::SubSpace(sub_space) => sub_space.sub_space.clone(),
                   })
               }
               received => Err(Fail::WrongResourceType { expected: HashSet::from_iter(vec![ResourceType::SubSpace,ResourceType::App,ResourceType::Artifact,ResourceType::File,ResourceType::FileSystem] ), received: received.resource_type().clone() }),
           }
       }

    */

    pub fn user(&self) -> Result<UserKey, Fail> {
        match self {
            ResourceKey::User(user) => Ok(user.clone()),
            received => Err(Fail::WrongResourceType {
                expected: HashSet::from_iter(vec![ResourceType::User]),
                received: received.resource_type().clone(),
            }),
        }
    }

    pub fn actor(&self) -> Result<ActorKey, Fail> {
        if let ResourceKey::Actor(key) = self {
            Ok(key.clone())
        } else {
            Err(Fail::WrongResourceType {
                expected: HashSet::from_iter(vec![ResourceType::Actor]),
                received: self.resource_type().clone(),
            })
        }
    }

    pub fn app(&self) -> Result<AppKey, Fail> {
        match self {
            ResourceKey::App(app) => Result::Ok(app.clone()),
            ResourceKey::Actor(actor) => ResourceKey::Actor(actor.clone()).parent().unwrap().app(),
            ResourceKey::FileSystem(filesystem) => match filesystem {
                FileSystemKey::App(app) => ResourceKey::FileSystem(FileSystemKey::App(app.clone()))
                    .parent()
                    .unwrap()
                    .app(),
                _ => Err(Fail::WrongResourceType {
                    expected: HashSet::from_iter(vec![
                        ResourceType::App,
                        ResourceType::Actor,
                        ResourceType::FileSystem,
                    ]),
                    received: self.resource_type().clone(),
                }),
            },
            _ => Err(Fail::WrongResourceType {
                expected: HashSet::from_iter(vec![
                    ResourceType::App,
                    ResourceType::Actor,
                    ResourceType::FileSystem,
                ]),
                received: self.resource_type().clone(),
            }),
        }
    }

    pub fn file(&self) -> Result<FileKey, Fail> {
        if let ResourceKey::File(key) = self {
            Ok(key.clone())
        } else {
            Err(Fail::WrongResourceType {
                expected: HashSet::from_iter(vec![ResourceType::File]),
                received: self.resource_type().clone(),
            })
        }
    }

    pub fn as_filesystem(&self) -> Result<FileSystemKey, Fail> {
        if let ResourceKey::FileSystem(key) = self {
            Ok(key.clone())
        } else {
            Err(Fail::WrongResourceType {
                expected: HashSet::from_iter(vec![ResourceType::FileSystem]),
                received: self.resource_type().clone(),
            })
        }
    }

    pub fn encode(&self) -> Result<String, Error> {
        Ok(base64::encode(self.bin()?))
    }

    pub fn decode(string: String) -> Result<Self, Error> {
        Ok(ResourceKey::from_bin(base64::decode(string)?)?)
    }

    /*
    pub fn manager(&self)->ResourceManagerKey
    {
        match self
        {
            ResourceKey::Nothing => ResourceManagerKey::Central,
            ResourceKey::Space(_) => ResourceManagerKey::Central,
            ResourceKey::SubSpace(sub_space) => {
                //ResourceManagerKey::Key(ResourceKey::Space(sub_space.space.clone()))
                ResourceManagerKey::Central
            }
            ResourceKey::App(app) => {
                //ResourceManagerKey::Key(ResourceKey::Space(app.sub_space.space.clone()))
                ResourceManagerKey::Central
            }
            ResourceKey::Actor(actor) => {
                ResourceManagerKey::Key(ResourceKey::App(actor.app.clone()))
            }
            ResourceKey::User(user) => {
                //ResourceManagerKey::Key(ResourceKey::Space(user.space.clone()))
                ResourceManagerKey::Central
            }
            ResourceKey::File(file) => {
                //ResourceManagerKey::Key(ResourceKey::App(file.app.clone()))
                ResourceManagerKey::Central
            }
            ResourceKey::FileSystem(key) => {
                match key
                {
                    FileSystemKey::App(app) => {
                        //ResourceManagerKey::Key(ResourceKey::Space(app.sub_space.space.clone()))
                        ResourceManagerKey::Central
                    }
                    FileSystemKey::SubSpace(sub_space) => {
                        //ResourceManagerKey::Key(ResourceKey::Space(app.sub_space.space.clone()))
                        ResourceManagerKey::Central
                    }
                }
            }
            ResourceKey::Domain(_) => {

            }
            ResourceKey::UrlPathPattern(_) => {}
            ResourceKey::Proxy(_) => {}
        }

    }
     */
}


impl ResourceSelectorId for ResourceKey {}


#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub struct DomainKey {
    pub space: SpaceKey,
    pub id: DomainId,
}

impl ToString for DomainKey {
    fn to_string(&self) -> String {
        format!("{}-{}", self.space.to_string(), self.id.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub struct ProxyKey {
    pub space: SpaceKey,
    pub id: ProxyId,
}

impl ToString for ProxyKey {
    fn to_string(&self) -> String {
        format!("{}-{}", self.space.to_string(), self.id.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub struct UrlPathPatternKey {
    pub domain: DomainKey,
    pub id: UrlPathPatternId,
}

impl ToString for UrlPathPatternKey {
    fn to_string(&self) -> String {
        format!("{}-{}", self.domain.to_string(), self.id.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub enum DatabaseKey {
    App(SubKey<AppKey, DatabaseId>),
    SubSpace(SubKey<SubSpaceKey, DatabaseId>),
}

impl DatabaseKey {
    pub fn id(&self) -> ResourceId {
        match self {
            Self::App(sub) => ResourceId::Database(sub.id.clone()),
            Self::SubSpace(sub) => ResourceId::Database(sub.id.clone()),
        }
    }
}

impl ToString for DatabaseKey{
    fn to_string(&self) -> String {
        match self {
            DatabaseKey::App(app) => {
                format!("app_{}", app.to_string())
            }
            DatabaseKey::SubSpace(sub_space) => {
                format!("subspace_{}", sub_space.to_string())
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub enum FileSystemKey {
    App(AppFilesystemKey),
    SubSpace(SubSpaceFilesystemKey),
}

impl FileSystemKey {
    pub fn id(&self) -> ResourceId {
        match self {
            FileSystemKey::App(app) => ResourceId::FileSystem(app.id.clone()),
            FileSystemKey::SubSpace(sub_space) => ResourceId::FileSystem(sub_space.id.clone()),
        }
    }
}

impl ToString for FileSystemKey {
    fn to_string(&self) -> String {
        match self {
            FileSystemKey::App(app) => {
                format!("app_{}", app.to_string())
            }
            FileSystemKey::SubSpace(sub_space) => {
                format!("subspace_{}", sub_space.to_string())
            }
        }
    }
}

impl FromStr for FileSystemKey {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut split = s.split("_");
        let sub_type = split.next().ok_or(
            format!("could not split string thought to be FileSystemKey: {}", s).to_string(),
        )?;
        match sub_type {
            "app" => Ok(FileSystemKey::App(AppFilesystemKey::from_str(
                split.next().ok_or("expected")?,
            )?)),
            "subspace" => Ok(FileSystemKey::SubSpace(SubSpaceFilesystemKey::from_str(
                split.next().ok_or("expected")?,
            )?)),
            what => Err(format!(
                "could not determine type of filsystem key for type {}",
                what
            )
            .into()),
        }
    }
}

impl FromStr for AppFilesystemKey {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let pos = s.rfind('-').ok_or("expected '-' between parent and id")?;
        let (parent, id) = s.split_at(pos);
        let app = AppKey::from_str(parent)?;
        let id = FileSystemId::from_str(id)?;
        Ok(AppFilesystemKey { app: app, id: id })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub struct SubKey<P:ToString, I:ToString> {
    pub parent: P,
    pub id: I,
}

impl <P:ToString,I:ToString> ToString for SubKey<P,I> {
    fn to_string(&self) -> String {
        format!("{}-{}", self.parent.to_string(), self.id.to_string())
    }
}

pub type FileSystemId = u32;

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub struct AppFilesystemKey {
    pub app: AppKey,
    pub id: FileSystemId,
}

impl ToString for AppFilesystemKey {
    fn to_string(&self) -> String {
        format!("{}-{}", self.app.to_string(), self.id.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub struct SubSpaceFilesystemKey {
    pub sub_space: SubSpaceKey,
    pub id: FileSystemId,
}

impl ToString for SubSpaceFilesystemKey {
    fn to_string(&self) -> String {
        format!("{}-{}", self.sub_space.to_string(), self.id.to_string())
    }
}

impl FromStr for SubSpaceFilesystemKey {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let pos = s.rfind('-').ok_or("expected '-' between parent and id")?;
        let (parent, id) = s.split_at(pos);
        let mut id = id.to_string();
        id.remove(0);
        let sub_space = SubSpaceKey::from_str(parent)?;

        let id = FileSystemId::from_str(id.as_str())?;
        Ok(SubSpaceFilesystemKey {
            sub_space: sub_space,
            id: id,
        })
    }
}

#[derive(Debug,Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub enum GatheringKey {
    Actor(ActorKey),
}

impl GatheringKey {
    pub fn bin(&self) -> Result<Vec<u8>, Error> {
        let mut bin = bincode::serialize(self)?;
        Ok(bin)
    }

    pub fn from_bin(mut bin: Vec<u8>) -> Result<GatheringKey, Error> {
        let mut key = bincode::deserialize::<GatheringKey>(bin.as_slice())?;
        Ok(key)
    }
}

impl fmt::Display for ResourceKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ResourceKey::Space(key) => format!("space-{}", key.to_string()),
                ResourceKey::SubSpace(key) => format!("sub_space-{}", key.to_string()),
                ResourceKey::App(key) => format!("app-{}", key.to_string()),
                ResourceKey::Actor(key) => format!("actor-{}", key.to_string()),
                ResourceKey::User(key) => format!("user-{}", key.to_string()),
                ResourceKey::File(key) => format!("file-{}", key.to_string()),
                ResourceKey::FileSystem(key) => format!("filesystem-{}", key.to_string()),
                ResourceKey::Root => "root".to_string(),
                ResourceKey::Domain(key) => format!("domain-{}", key.to_string()),
                ResourceKey::UrlPathPattern(key) => format!("url-path-pattern-{}", key.to_string()),
                ResourceKey::Proxy(key) => format!("proxy-{}", key.to_string()),
                ResourceKey::ArtifactBundle(key) => format!("artifact_bundle-{}", key.to_string()),
                ResourceKey::Artifact(key) => format!("artifact-{}", key.to_string()),
                ResourceKey::Database(key) => format!("database-{}", key.to_string()),
            }
        )
    }
}

impl ResourceKey {
    pub fn resource_type(&self) -> ResourceType {
        match self {
            ResourceKey::Root => ResourceType::Root,
            ResourceKey::Space(_) => ResourceType::Space,
            ResourceKey::SubSpace(_) => ResourceType::SubSpace,
            ResourceKey::App(_) => ResourceType::App,
            ResourceKey::Actor(_) => ResourceType::Actor,
            ResourceKey::User(_) => ResourceType::User,
            ResourceKey::File(_) => ResourceType::File,
            ResourceKey::FileSystem(_) => ResourceType::FileSystem,
            ResourceKey::Domain(_) => ResourceType::Domain,
            ResourceKey::UrlPathPattern(_) => ResourceType::UrlPathPattern,
            ResourceKey::Proxy(_) => ResourceType::Proxy,
            ResourceKey::ArtifactBundle(_) => ResourceType::ArtifactBundle,
            ResourceKey::Artifact(_) => ResourceType::Artifact,
            ResourceKey::Database(_) => ResourceType::Database
        }
    }

    pub fn sub_space(&self) -> Result<SubSpaceKey, Error> {
        match self {
            ResourceKey::Root => Err("Root does not have a subspace".into()),
            ResourceKey::Space(_) => Err("space does not have a subspace".into()),
            ResourceKey::SubSpace(sub_space) => Ok(sub_space.clone()),
            ResourceKey::App(app) => Ok(app.sub_space.clone()),
            ResourceKey::Actor(actor) => Ok(actor.app.sub_space.clone()),
            ResourceKey::User(user) => Err("user does not have a sub_space".into()),
            ResourceKey::File(file) => match &file.filesystem {
                FileSystemKey::App(app) => Ok(app.app.sub_space.clone()),
                FileSystemKey::SubSpace(sub_space) => Ok(sub_space.sub_space.clone()),
            },
            ResourceKey::FileSystem(filesystem) => match filesystem {
                FileSystemKey::App(app) => Ok(app.app.sub_space.clone()),
                FileSystemKey::SubSpace(sub_space) => Ok(sub_space.sub_space.clone()),
            },
            ResourceKey::Domain(_) => Err("Domain does not have a subspace".into()),
            ResourceKey::UrlPathPattern(_) => Err("UrlPathPattern does not have a subspace".into()),
            ResourceKey::Proxy(_) => Err("Proxy does not have a subspace".into()),
            ResourceKey::ArtifactBundle(bundle) => Ok(bundle.sub_space.clone()),
            ResourceKey::Artifact(artifact) => Ok(artifact.bundle.sub_space.clone()),
            ResourceKey::Database(filesystem) => match filesystem {
                DatabaseKey::App(app) => Ok(app.parent.sub_space.clone()),
                DatabaseKey::SubSpace(sub_space) => Ok(sub_space.parent.clone()),
            },
        }
    }

    pub fn bin(&self) -> Result<Vec<u8>, Error> {
        let mut bin = bincode::serialize(self)?;
        bin.insert(0, self.resource_type().magic());
        Ok(bin)
    }

    pub fn from_bin(mut bin: Vec<u8>) -> Result<ResourceKey, Error> {
        bin.remove(0);
        let mut key = bincode::deserialize::<ResourceKey>(bin.as_slice())?;
        Ok(key)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct FileKey {
    pub filesystem: FileSystemKey,
    pub id: FileId,
}

impl FileKey {
    pub fn new(filesystem: FileSystemKey, id: FileId) -> Self {
        FileKey {
            filesystem: filesystem,
            id: id,
        }
    }
}

impl ToString for FileKey {
    fn to_string(&self) -> String {
        format!("{}-{}", self.filesystem.to_string(), self.id.to_string())
    }
}

impl FromStr for FileKey {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let pos = s.rfind('-').ok_or("expected '-' between parent and id")?;
        let (parent, id) = s.split_at(pos);
        let filesystem = FileSystemKey::from_str(parent)?;
        let id = FileId::from_str(id)?;
        Ok(FileKey {
            filesystem: filesystem,
            id: id,
        })
    }
}

pub type FileId = u64;

#[derive(Clone, Serialize, Deserialize)]
pub enum Unique {
    Sequence,
    Index,
}

#[async_trait]
pub trait UniqueSrc: Send + Sync {
    async fn next(&self, resource_type: &ResourceType) -> Result<ResourceId, Fail>;
}
