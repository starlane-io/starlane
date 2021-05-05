use std::fmt;
use serde::{Deserialize, Serialize, Serializer};


pub type SpaceKey =u64;

#[derive(Clone,Serialize,Deserialize,Hash,Eq,PartialEq)]
pub struct UserKey
{
  pub space: SpaceKey,
  pub id: u64
}

impl UserKey
{
    pub fn new(space: SpaceKey, id: u64 ) -> Self
    {
        UserKey{
            space,
            id: id
        }
    }
}

#[derive(Clone,Serialize,Deserialize,Eq,PartialEq,Hash)]
pub struct SubSpaceKey
{
    pub space: SpaceKey,
    pub id: u16
}

impl SubSpaceKey
{
    pub fn main( ) -> Self
    {
        SubSpaceKey::new( 0, 0 )
    }

    pub fn new( space: SpaceKey, id: u16 ) -> Self
    {
        SubSpaceKey{
            space: space,
            id: id
        }
    }
}


#[derive(Clone,Hash,Eq,PartialEq,Serialize,Deserialize)]
pub struct AppKey
{
    pub sub_space: SubSpaceKey,
    pub id: u64
}

impl fmt::Display for SubSpaceKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.id)
    }
}

impl AppKey
{
    pub fn new(sub_space: SubSpaceKey, id: u64)->Self
    {
        AppKey{
            sub_space: sub_space,
            id: id
        }
    }
}

impl fmt::Display for AppKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({},{})", self.sub_space, self.id)
    }
}


