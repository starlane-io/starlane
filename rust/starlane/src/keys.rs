use std::fmt;
use serde::{Deserialize, Serialize, Serializer};
use uuid::Uuid;
use crate::user::Priviledges;


#[derive(Clone,Serialize,Deserialize,Hash,Eq,PartialEq)]
pub enum SpaceKey
{
    Hyper,
    Space(String)
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
        UserKey::with_id(SpaceKey::Hyper,UserId::Super)
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
            SpaceKey::Hyper => {
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
        SubSpaceKey::new( SpaceKey::Hyper, SubSpaceId::Default )
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
    Uuid(Uuid)
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
    Hyper,
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
            AppId::Hyper => "Hyper".to_string(),
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
            SubSpaceId::Uuid(uuid) => uuid.to_string()
        };
        write!(f, "{}", str )
    }
}

pub type MessageId = Uuid;
