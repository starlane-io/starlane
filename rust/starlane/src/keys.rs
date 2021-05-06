use std::fmt;
use serde::{Deserialize, Serialize, Serializer};
use uuid::Uuid;


#[derive(Clone,Serialize,Deserialize,Hash,Eq,PartialEq)]
pub enum SpaceKey
{
    Main,
    Named(String)
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
}

#[derive(Clone,Serialize,Deserialize,Hash,Eq,PartialEq)]
pub enum UserId
{
    Super,
    Developer,
    User,
    Guest,
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
    pub fn main( ) -> Self
    {
        SubSpaceKey::new( SpaceKey::Main, SubSpaceId::Default )
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
    System,
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
            AppId::System => "System".to_string(),
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
