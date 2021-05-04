use crate::app::{AppCreate, AppSelect};
use crate::keys::{OrgKey, UserKey};
use serde::{Deserialize, Serialize, Serializer};

#[derive(Clone,Serialize,Deserialize)]
pub struct OrgCommandWrapper
{
    org: OrgKey,
    user: UserKey,
    command: OrgCommand
}

#[derive(Clone,Serialize,Deserialize)]
pub enum OrgCommand
{
    AppCreate(AppCreate),
    AppSelect(AppSelect),
    Destroy
}
