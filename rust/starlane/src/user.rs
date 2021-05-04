use crate::error::Error;
use crate::label::Labels;
use std::collections::HashSet;
use serde::{Deserialize, Serialize, Serializer};
use crate::keys::UserKey;

#[derive(Clone,Serialize,Deserialize,Eq,PartialEq)]
pub struct AuthToken
{
    user: User,
    valid: bool
}

impl AuthToken
{
    pub fn new( user: User ) -> Self
    {
        AuthToken {
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
                kind == user.kind
            }
            UserPattern::Exact(name) => {
                if name == user.name {
                    true
                }
                else {
                    false
                }
            }
        }
    }
}

#[derive(Clone,Serialize,Deserialize,Eq,PartialEq,Hash)]
pub struct Permissions
{
    pub patterns: Vec<UserPattern>,
    pub access: HashSet<AccessPriv>
}

impl Permissions
{
    pub fn is_permitted( &self, user: &User ) -> bool
    {
        for pattern in &self.patterns
        {
            if pattern.is_match(user)
            {
                true
            }
        }
        false
    }
}

#[derive(Clone,Serialize,Deserialize,Eq,PartialEq,Hash)]
pub enum AccessPriv
{
    App(AppAccess),
    Actor(ActorAccess)
}

#[derive(Clone,Serialize,Deserialize,Eq,PartialEq,Hash)]
pub enum AppAccess
{
    Create,
    Message,
    Watch,
    Destroy
}

#[derive(Clone,Serialize,Deserialize,Eq,PartialEq,Hash)]
pub enum ActorAccess
{
    Create,
    Watch,
    Message(MessagePortAccess),
    Destroy
}

#[derive(Clone,Serialize,Deserialize,Eq,PartialEq,Hash)]
pub enum MessagePortAccess
{
    Any,
    Exact(String),
}