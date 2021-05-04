use std::fmt;
use serde::{Deserialize, Serialize, Serializer};


#[derive(Clone,Serialize,Deserialize,Hash,Eq,PartialEq)]
pub struct TenantKey
{
    pub org: OrgKey,
    pub group: GroupKey
}

impl TenantKey
{
    pub(crate) fn new(org: OrgKey, group: GroupKey ) -> Self
    {
       TenantKey{
           org: org,
           group: group
       }
    }
}

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

#[derive(Clone,Serialize,Deserialize,Eq,PartialEq,Hash)]
pub struct GroupKey
{
    pub id: u16
}

pub type OrgKey=u64;

#[derive(Clone,Hash,Eq,PartialEq)]
pub struct AppKey
{
    pub tenant: TenantKey,
    pub id: u64
}



impl AppKey
{
    pub fn new(tenant: TenantKey, id: u64)->Self
    {
        AppKey{
            tenant: tenant,
            id: id
        }
    }
}

impl fmt::Display for AppKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({},{})", self.app, self.id)
    }
}