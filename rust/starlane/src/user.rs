use crate::org::OrgKey;
use crate::label::Labels;
use crate::error::Error;

#[derive(Clone,Serialize,Deserialize,Hash,Eq,PartialEq)]
pub struct UserKey
{
  pub org: OrgKey,
  pub id: u64
}

impl UserKey
{
    pub fn new( org: OrgKey, id: u64 ) -> Self
    {
        UserKey{
            org: org,
            id: id
        }
    }
}

#[derive(Clone,Serialize,Deserialize,Eq,PartialEq)]
pub struct UserToken
{
    user: User,
    valid: bool
}

impl UserToken
{
    pub fn new( user: User ) -> Self
    {
        UserToken{
            user: user,
            valid: true
        }
    }

    pub fn get_user(&self)->Result<User,TokenError>
    {
        if !&self.valid
        {
            Err(TokenError::Invalid)
        }

        Ok(self.user.clone())
    }

    pub fn is_valid(&self) -> bool
    {
        self.valid.clone()
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
    pub labels: Option<Labels>
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
pub struct GroupKey
{
    pub org: OrgKey,
    pub id: u64
}

pub struct Group
{
    pub name: String
}