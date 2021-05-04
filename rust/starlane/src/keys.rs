use starlane::org::OrgKey;
use starlane::user::GroupKey;

#[derive(Clone,Serialize,Deserialize,Hash,Eq,PartialEq)]
pub struct TenantKey
{
    pub org: OrgKey,
    pub group: GroupKey
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
    pub org: OrgKey,
    pub id: u16
}

pub type OrgKey=u64;
