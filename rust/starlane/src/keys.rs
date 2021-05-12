use std::fmt;

use serde::{Deserialize, Serialize, Serializer};
use uuid::Uuid;

use crate::actor::{Actor, ActorKey, ActorKind, ActorRef};
use crate::app::{App, AppKind};
use crate::artifact::{ArtifactKey, ArtifactKind};
use crate::filesystem::FileKey;
use crate::names::Name;
use crate::permissions::{Priviledges, User, UserKind};
use std::str::FromStr;
use crate::error::Error;
use crate::label::Labels;

#[derive(Clone,Serialize,Deserialize,Hash,Eq,PartialEq)]
pub enum SpaceKey
{
    HyperSpace,
    Space(u32)
}

impl SpaceKey
{

    pub fn from_index(index: u32) -> Self
    {
        if index == 0
        {
            SpaceKey::HyperSpace
        }
        else
        {
            SpaceKey::Space(index)
        }
    }

    pub fn index(&self)->u32
    {
        match self
        {
            SpaceKey::HyperSpace => 0,
            SpaceKey::Space(index) => index.clone()
        }
    }
}

#[derive(Clone,Serialize,Deserialize,Hash,Eq,PartialEq)]
pub struct UserKey
{
  pub space: SpaceKey,
  pub id: UserId
}

impl UserKey
{
    pub fn new(space: SpaceKey) -> Self
    {
        UserKey{
            space,
            id: UserId::new()
        }
    }

    pub fn with_id(space: SpaceKey, id: UserId) -> Self
    {
        UserKey{
            space,
            id: id
        }
    }

    pub fn hyperuser() -> Self
    {
        UserKey::with_id(SpaceKey::HyperSpace, UserId::Super)
    }


    pub fn superuser(space: SpaceKey) -> Self
    {
        UserKey::with_id(space,UserId::Super)
    }

    pub fn annonymous(space: SpaceKey) -> Self
    {
        UserKey::with_id(space,UserId::Annonymous)
    }


    pub fn is_hyperuser(&self)->bool
    {
        match self.space{
            SpaceKey::HyperSpace => {
                match self.id
                {
                    UserId::Super => true,
                    _ => false
                }
            }
            _ => false
        }
    }

    pub fn privileges(&self) -> Priviledges
    {
        if self.is_hyperuser()
        {
            Priviledges::all()
        }
        else {
            Priviledges::new()
        }
    }
}

#[derive(Clone,Serialize,Deserialize,Hash,Eq,PartialEq)]
pub enum UserId
{
    Super,
    Annonymous,
    Uuid(Uuid)
}

impl UserId
{
    pub fn new()->Self
    {
        Self::Uuid(Uuid::new_v4())
    }
}

#[derive(Clone,Serialize,Deserialize,Eq,PartialEq,Hash)]
pub struct SubSpaceKey
{
    pub space: SpaceKey,
    pub id: SubSpaceId
}

impl SubSpaceKey
{
    pub fn hyper_default( ) -> Self
    {
        SubSpaceKey::new(SpaceKey::HyperSpace, SubSpaceId::Default )
    }

    pub fn new( space: SpaceKey, id: SubSpaceId ) -> Self
    {
        SubSpaceKey{
            space: space,
            id: id
        }
    }
}


#[derive(Clone,Serialize,Deserialize,Eq,PartialEq,Hash)]
pub enum SubSpaceId
{
    Default,
    Index(u32)
}

impl SubSpaceId
{
    pub fn from_index(index: u32) -> Self
    {
        if index == 0
        {
            Self::Default
        }
        else
        {
            Self::Index(index)
        }
    }

    pub fn index(&self)->u32
    {
        match self
        {
            SubSpaceId::Default => 0,
            SubSpaceId::Index(index) => index.clone()
        }
    }
}


#[derive(Clone,Hash,Eq,PartialEq,Serialize,Deserialize)]
pub struct AppKey
{
    pub sub_space: SubSpaceKey,
    pub id: AppId
}


#[derive(Clone,Hash,Eq,PartialEq,Serialize,Deserialize)]
pub enum AppId
{
    HyperApp,
    Uuid(Uuid)
}

impl AppId
{
    pub fn new()->Self
    {
        Self::Uuid(Uuid::new_v4())
    }
}

impl fmt::Display for SubSpaceKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.id)
    }
}

impl AppKey
{
    pub fn new( sub_space: SubSpaceKey )->Self
    {
        AppKey{
            sub_space: sub_space,
            id: AppId::new()
        }
    }
}

impl fmt::Display for AppKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({},{})", self.sub_space, self.id)
    }
}

impl fmt::Display for AppId{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let fmt = match self
        {
            AppId::HyperApp => "HyperApp".to_string(),
            AppId::Uuid(uuid) => uuid.to_string()
        };
        write!(f, "{}", fmt )
    }
}


impl fmt::Display for SubSpaceId{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let str = match self
        {
            SubSpaceId::Default => "Default".to_string(),
            SubSpaceId::Index(index) => index.to_string()
        };
        write!(f, "{}", str )
    }
}

pub type MessageId = Uuid;

#[derive(Clone,Serialize,Deserialize,Hash,Eq,PartialEq)]
pub enum ResourceKey
{
    Space(SpaceKey),
    SubSpace(SubSpaceKey),
    App(AppKey),
    Actor(ActorKey),
    User(UserKey),
    File(FileKey),
    Artifact(ArtifactKey)
}

impl ResourceKey
{
    pub fn resource_type(&self) -> ResourceType
    {
        match self
        {
            ResourceKey::Space(_) => ResourceType::Space,
            ResourceKey::SubSpace(_) => ResourceType::SubSpace,
            ResourceKey::App(_) => ResourceType::App,
            ResourceKey::Actor(_) => ResourceType::Actor,
            ResourceKey::User(_) => ResourceType::User,
            ResourceKey::File(_) => ResourceType::File,
            ResourceKey::Artifact(_) => ResourceType::Artifact
        }
    }

    pub fn sub_space(&self)->SubSpaceKey
    {
        match self
        {
            ResourceKey::Space(_) => SubSpaceKey::hyper_default(),
            ResourceKey::SubSpace(_) => SubSpaceKey::hyper_default(),
            ResourceKey::App(app) => app.sub_space.clone(),
            ResourceKey::Actor(actor) => actor.app.sub_space.clone(),
            ResourceKey::User(user) => SubSpaceKey::new( user.space.clone(), SubSpaceId::Default ),
            ResourceKey::File(file) => file.sub_space.clone(),
            ResourceKey::Artifact(artifact) => artifact.sub_space.clone()
        }
    }




    pub fn space(&self)->SpaceKey
    {
        self.sub_space().space
    }
}

#[derive(Clone,Serialize,Deserialize,Hash,Eq,PartialEq)]
pub enum ResourceKind
{
    Space,
    SubSpace,
    App(AppKind),
    Actor(ActorKind),
    User,
    File,
    Artifact(ArtifactKind)
}

impl fmt::Display for ResourceKind{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!( f,"{}",
                match self{
                    ResourceKind::Space=> "Space".to_string(),
                    ResourceKind::SubSpace=> "SubSpace".to_string(),
                    ResourceKind::App(kind)=> format!("App:{}",kind).to_string(),
                    ResourceKind::Actor(kind)=> format!("Actor:{}",kind).to_string(),
                    ResourceKind::User=> "User".to_string(),
                    ResourceKind::File=> "File".to_string(),
                    ResourceKind::Artifact(kind)=>format!("Artifact:{}",kind).to_string()
                })
    }
}

impl FromStr for ResourceKind
{
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {

        if s.starts_with("App:") {
            let mut split = s.split(":");
            split.next().ok_or("error")?;
            return Ok( ResourceKind::App( AppKind::from_str(split.next().ok_or("error")?)? ));
        } else if s.starts_with("Actor:") {
            let mut split = s.split(":");
            split.next().ok_or("error")?;
            return Ok( ResourceKind::Actor( ActorKind::from_str(split.next().ok_or("error")?)? ) );
        } else if s.starts_with("Artifact:") {
            let mut split = s.split(":");
            split.next().ok_or("error")?;
            return Ok( ResourceKind::Artifact( ArtifactKind::from_str(split.next().ok_or("error")?)? ) );
        }


        match s
        {
            "Space" => Ok(ResourceKind::Space),
            "SubSpace" => Ok(ResourceKind::SubSpace),
            "User" => Ok(ResourceKind::User),
            "File" => Ok(ResourceKind::File),
            _ => {
                Err(format!("cannot match ResourceKind: {}", s).into())
            }
        }
    }
}
#[derive(Clone,Serialize,Deserialize,Hash,Eq,PartialEq)]
pub enum ResourceType
{
    Space,
    SubSpace,
    App,
    Actor,
    User,
    File,
    Artifact
}

impl ResourceType
{
    pub fn has_specific(&self)->bool
    {
        match self
        {
            ResourceType::Space => false,
            ResourceType::SubSpace => false,
            ResourceType::App => true,
            ResourceType::Actor => true,
            ResourceType::User => false,
            ResourceType::File => false,
            ResourceType::Artifact => true
        }
    }
}


impl fmt::Display for ResourceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!( f,"{}",
                match self{
                    ResourceType::Space=> "Space".to_string(),
                    ResourceType::SubSpace=> "SubSpace".to_string(),
                    ResourceType::App=> "App".to_string(),
                    ResourceType::Actor=> "Actor".to_string(),
                    ResourceType::User=> "User".to_string(),
                    ResourceType::File=> "File".to_string(),
                    ResourceType::Artifact=> "Artifact".to_string(),
                })
    }
}



pub struct ResourceMeta
{
    name: Option<String>,
    labels: Labels
}

pub struct Resource
{
    pub key: ResourceKey,
    pub owner: UserKey,
    pub kind: ResourceKind,
    pub specific: Option<Name>,
}

impl Resource
{
    pub fn app(&self)->Option<AppKey>
    {
        match &self.key
        {
            ResourceKey::Space(_) => Option::None,
            ResourceKey::SubSpace(_) => Option::None,
            ResourceKey::App(_) => Option::None,
            ResourceKey::Actor(actor) => {
                Option::Some(actor.app.clone())
            }
            ResourceKey::User(_) => Option::None,
            ResourceKey::File(_) => Option::None,
            ResourceKey::Artifact(_) => Option::None
        }
    }
}

impl From<AppKind> for ResourceKind{
    fn from(e: AppKind) -> Self {
        ResourceKind::App(e)
    }
}

impl From<App> for ResourceKind{
    fn from(e: App) -> Self {
        ResourceKind::App(e.archetype.kind)
    }
}

impl From<ActorKind> for ResourceKind{
    fn from(e: ActorKind) -> Self {
        ResourceKind::Actor(e)
    }
}

impl From<ArtifactKind> for ResourceKind{
    fn from(e: ArtifactKind) -> Self {
        ResourceKind::Artifact(e)
    }
}

impl From<App> for Resource{
    fn from(e: App) -> Self {
        Resource{
            key: ResourceKey::App(e.key.clone()),
            specific: Option::Some(e.archetype.specific.clone()),
            owner: e.archetype.owner.clone(),
            kind: e.into()
        }
    }
}

impl From<ActorRef> for Resource{
    fn from(e: ActorRef) -> Self {
        Resource{
            key: ResourceKey::Actor(e.key),
            specific: Option::Some(e.archetype.specific),
            owner: e.archetype.owner,
            kind: e.archetype.kind.into()
        }
    }
}

impl From<User> for Resource{
    fn from(e: User) -> Self {
        Resource{
            key: ResourceKey::User(e.key.clone()),
            specific: Option::None,
            owner: e.key,
            kind: ResourceKind::User
        }
    }
}

impl From<SpaceKey> for Resource{
    fn from(e: SpaceKey) -> Self {
        Resource{
            key: ResourceKey::Space(e),
            specific: Option::None,
            owner: UserKey::hyperuser(),
            kind: ResourceKind::Space
        }
    }
}

