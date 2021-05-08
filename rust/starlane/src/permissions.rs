use std::collections::HashSet;

use serde::{Deserialize, Serialize, Serializer};

use crate::crypt::{JwtDecoder, PrivateKey};
use crate::error::Error;
use crate::keys::{SpaceKey, UserId, UserKey, ResourceKey};
use crate::label::Labels;

#[derive(Clone,Serialize,Deserialize,Eq,PartialEq)]
pub struct AuthToken
{
    pub user: User
}

impl AuthToken
{
    pub fn decode( &self, decoder: JwtDecoder )-> Authentication
    {
        unimplemented!();
    }
}

#[derive(Clone)]
pub struct Authentication
{
   pub user: UserKey
}

impl Authentication
{
    pub fn mock(user: UserKey)->Self
    {
        Authentication{
            user: user
        }
    }
}

pub enum TokenError
{
    Error(Error),
    Invalid
}

#[derive(Clone,Serialize,Deserialize,Eq,PartialEq)]
pub struct User
{
    pub name: String,
    pub key: UserKey,
    pub labels: Option<Labels>,
    pub kind: UserKind
}

#[derive(Clone,Serialize,Deserialize,Eq,PartialEq,Hash)]
pub enum UserKind
{
    Super,
    Developer,
    User,
    Guest,
    Custom(String)
}

#[derive(Clone,Serialize,Deserialize,Eq,PartialEq,Hash)]
pub enum UserRole
{
    Owner,
    Developer,
    User,
    Guest,
    Observer
}

pub struct RoleBinding
{
    user: UserKey,
    resource: ResourceKey,
    role: UserRole
}

pub struct Grant
{
   pub user: UserKey,
   pub resource: ResourceKey,
   pub priviledge: Priviledge
}

pub enum Priviledge
{
    Access(HashSet<Access>),
    Role(UserRole)
}

#[derive(Clone,Serialize,Deserialize,Eq,PartialEq,Hash)]
pub enum UserPattern
{
    None,
    Any,
    Exact(String),
    Kind(UserKind)
}

impl UserPattern
{
    pub fn is_match( &self, user: &User) -> bool
    {
        match &self
        {
            UserPattern::None => {
                false
            }
            UserPattern::Any => {
                true
            }
            UserPattern::Kind(kind) => {
                *kind == user.kind
            }
            UserPattern::Exact(name) => {
                if *name == user.name {
                    true
                }
                else {
                    false
                }
            }
        }
    }
}

#[derive(Clone,Serialize,Deserialize,Eq,PartialEq)]
pub struct Permissions
{
    pub patterns: Vec<UserPattern>,
    pub access: HashSet<Access>
}

impl Permissions
{
    pub fn is_permitted( &self, user: &User ) -> bool
    {
        for pattern in &self.patterns
        {
            if pattern.is_match(user)
            {
                return true;
            }
        }
        return false;
    }
}

pub struct Priviledges
{
    pub hyper: HashSet<HyperSpaceAccess>,
    pub space: HashSet<SpaceAccess>,
    pub app: HashSet<AppAccess>,
    pub actor: HashSet<ActorAccess>
}

impl Priviledges
{
    pub fn new() -> Self
    {
        Priviledges {
            hyper: HashSet::new(),
            space: HashSet::new(),
            app: HashSet::new(),
            actor: HashSet::new()
        }
    }

    pub fn all() -> Self
    {
        let mut rtn = Self::new();
        rtn.hyper.union(&HyperSpaceAccess::all().into_iter().collect() );
        rtn.space.union(&SpaceAccess::all().into_iter().collect() );
        rtn.app.union(&AppAccess::all().into_iter().collect() );
        rtn.actor.union(&ActorAccess::all().into_iter().collect() );

        rtn
    }

    pub fn new_union(&self, other: &Priviledges) -> Self
    {
        let mut rtn = Priviledges::new();
        rtn.union(self);
        rtn.union(other);
        rtn
    }

    pub fn union( &mut self, other: &Priviledges)
    {
        self.hyper.union( &other.hyper.clone() );
        self.space.union( &other.space.clone() );
        self.app.union( &other.app.clone() );
        self.actor.union( &other.actor.clone() );
    }

}



#[derive(Clone,Serialize,Deserialize,Eq,PartialEq,Hash)]
pub enum Access
{
    Hyper(HyperSpaceAccess),
    Space(SpaceAccess),
    App(AppAccess),
    Actor(ActorAccess)
}

#[derive(Clone,Serialize,Deserialize,Eq,PartialEq,Hash)]
pub enum AppAccess
{
    CreateActor,
    DestroyActor,
    DestroyApp,
    Watch,
}

impl AppAccess
{
    pub fn all() -> Vec<Self>
    {
        vec![Self::CreateActor, Self::DestroyActor, Self::DestroyApp, Self::Watch]
    }
    pub fn role(role: UserRole) -> Vec<Self>
    {
        match role
        {
            UserRole::Owner => Self::all(),
            UserRole::Developer => {
                vec![Self::CreateActor, Self::DestroyActor, Self::Watch]
            }
            UserRole::User => {
                vec![Self::CreateActor, Self::Watch]
            }
            _ => {
                vec![Self::Watch]
            }
        }
    }
}

#[derive(Clone,Serialize,Deserialize,Eq,PartialEq,Hash)]
pub enum HyperSpaceAccess
{
    CreateSpaces,
    DestroySpaces,
    ViewSpaces,
    ElevateToHyperUser,
}

impl HyperSpaceAccess
{
    pub fn all()->Vec<Self>
    {
        vec![Self::CreateSpaces,Self::ViewSpaces,Self::DestroySpaces,Self::ElevateToHyperUser]
    }

    pub fn role(role: UserRole)->Vec<Self>
    {
        match role
        {
            UserRole::Owner => {
                Self::all()
            }
            UserRole::Developer => {
                vec![Self::CreateSpaces,Self::ViewSpaces,Self::DestroySpaces,Self::ElevateToHyperUser]
            }
            UserRole::User => {
                vec![Self::CreateSpaces,Self::ViewSpaces,Self::ElevateToHyperUser]
            }
            UserRole::Guest => {
                vec![Self::ViewSpaces]
            }
            _ => {
                vec![]
            }
        }
    }
}

#[derive(Clone,Serialize,Deserialize,Eq,PartialEq,Hash)]
pub enum SpaceAccess
{
    CreateUser,
    ModifyUser,
    ViewUser,
    DestroyUser,
    CreateSubSpace,
    ViewSubSpace,
    DestroySubSpace,
    ElevateToSuperUser,
}

impl SpaceAccess
{
    pub fn all()->Vec<Self>
    {
        vec![Self::CreateUser,Self::ModifyUser,Self::ViewUser,Self::DestroyUser,Self::CreateSubSpace,Self::ViewSubSpace,Self::DestroySubSpace,Self::ElevateToSuperUser]
    }

    pub fn role(role: UserRole)->Vec<Self>
    {
        match role
        {
            UserRole::Owner => {
                Self::all()
            }
            UserRole::Developer => {
                vec![Self::CreateUser,Self::ModifyUser,Self::ViewUser,Self::DestroyUser,Self::CreateSubSpace,Self::ViewSubSpace,Self::DestroySubSpace]
            }
            UserRole::User => {
                vec![Self::CreateUser,Self::ModifyUser,Self::ViewUser,Self::CreateSubSpace,Self::ViewSubSpace]
            }
            _ => {
                vec![Self::ViewUser,Self::ViewSubSpace]
            }
        }
    }
}

#[derive(Clone,Serialize,Deserialize,Eq,PartialEq,Hash)]
pub enum ActorAccess
{
    Create,
    Modify,
    Message,
    Watch,
    Destroy
}

impl ActorAccess
{
    pub fn all()->Vec<Self>
    {
        vec![Self::Create,Self::Watch,Self::Message,Self::Modify,Self::Destroy]
    }

    pub fn role(role: UserRole)->Vec<Self>
    {
        match role
        {
            UserRole::Owner => {
                Self::all()
            }
            UserRole::Developer => {
                vec![Self::Create,Self::Modify,Self::Message,Self::Watch,Self::Destroy]
            }
            UserRole::User => {
                vec![Self::Watch,Self::Message,Self::Modify]
            }
            UserRole::Guest => {
                vec![Self::Watch,Self::Message]
            }
            UserRole::Observer => {
                vec![Self::Watch]
            }
        }
    }
}

#[derive(Clone)]
pub struct AuthTokenSource
{

}



impl AuthTokenSource
{
    pub fn new()->Self
    {
        AuthTokenSource{}
    }

    pub async fn auth(&self, creds: &Credentials ) -> Result<AuthToken,Error>
    {
        Ok(AuthToken{
            user: User{
                name: "someuser".to_string(),
                key: creds.user.clone(),
                labels: None,
                kind: match creds.user.id{
                    UserId::Super => UserKind::Super,
                    UserId::Annonymous => UserKind::Guest,
                    UserId::Uuid(_) => UserKind::User
                }
            }
        })
    }
}

#[derive(Clone)]
pub struct Credentials
{
    pub user: UserKey
}

impl Credentials
{
    pub fn mock( user: UserKey ) -> Self
    {
        Credentials{
            user: user
        }
    }
}

